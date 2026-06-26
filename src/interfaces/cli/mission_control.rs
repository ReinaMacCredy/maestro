use anyhow::{Context, Result, bail};

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{MissionControlArgs, MissionControlFormat};
use crate::interfaces::tui::mission_control::{
    PreviewFormat, PreviewScreen, RenderOptions, render_check, render_preview, snapshot,
};

pub fn run(args: MissionControlArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let size = parse_size(args.size.as_deref())?;

    if args.json {
        let snapshot = snapshot(&paths)?;
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }

    if args.render_check {
        let result = render_check(&paths, size)?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let screen = resolve_screen(args.preview, args.screen.as_deref())?;
    let format = match args.format.unwrap_or(MissionControlFormat::Plain) {
        MissionControlFormat::Plain => PreviewFormat::Plain,
        MissionControlFormat::Ansi => PreviewFormat::Ansi,
    };
    let frame = render_preview(
        &paths,
        RenderOptions {
            screen,
            feature: args.feature.as_deref(),
            width: size.map(|(width, _)| width),
            height: size.map(|(_, height)| height),
            format,
        },
    )?;
    print!("{frame}");
    Ok(())
}

fn resolve_screen(
    preview: Option<Option<String>>,
    screen: Option<&str>,
) -> Result<Option<PreviewScreen>> {
    let raw = match (screen, preview) {
        (Some(screen), None) => Some(screen.to_string()),
        (None, Some(Some(preview))) => Some(preview),
        (None, Some(None)) => Some("dashboard".to_string()),
        (None, None) => Some("dashboard".to_string()),
        (Some(_), Some(_)) => bail!("choose either --screen or --preview"),
    };
    raw.as_deref().map(PreviewScreen::parse).transpose()
}

fn parse_size(value: Option<&str>) -> Result<Option<(usize, usize)>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let (width, height) = value
        .split_once('x')
        .or_else(|| value.split_once('X'))
        .with_context(|| format!("invalid --size '{value}'; use WxH, e.g. 120x40"))?;
    let width = width
        .parse::<usize>()
        .with_context(|| format!("invalid width in --size '{value}'"))?;
    let height = height
        .parse::<usize>()
        .with_context(|| format!("invalid height in --size '{value}'"))?;
    if width < 40 || height < 20 {
        bail!("size {width}x{height} is too small; minimum is 40x20");
    }
    Ok(Some((width, height)))
}

#[cfg(test)]
mod tests {
    use super::{parse_size, resolve_screen};
    use crate::interfaces::tui::mission_control::PreviewScreen;

    #[test]
    fn parse_size_accepts_wxh_and_rejects_too_small() {
        assert_eq!(parse_size(Some("120x40")).unwrap(), Some((120, 40)));
        assert!(parse_size(Some("39x20")).is_err());
        assert!(parse_size(Some("wide")).is_err());
    }

    #[test]
    fn resolve_screen_defaults_to_dashboard_and_accepts_aliases() {
        assert_eq!(
            resolve_screen(None, None).unwrap(),
            Some(PreviewScreen::Dashboard)
        );
        assert_eq!(
            resolve_screen(Some(Some("task".to_string())), None).unwrap(),
            Some(PreviewScreen::Tasks)
        );
        assert!(resolve_screen(Some(Some("unknown".to_string())), None).is_err());
    }
}
