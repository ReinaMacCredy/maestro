use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{MissionControlArgs, MissionControlFormat, MissionControlRenderer};
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

    if should_use_opentui(&args) {
        run_opentui(&paths, &args, size)?;
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

fn should_use_opentui(args: &MissionControlArgs) -> bool {
    if args.renderer == Some(MissionControlRenderer::Opentui) {
        return true;
    }
    args.renderer.is_none()
        && args.preview.is_none()
        && args.screen.is_none()
        && !args.render_check
        && !args.json
}

fn run_opentui(
    paths: &MaestroPaths,
    args: &MissionControlArgs,
    size: Option<(usize, usize)>,
) -> Result<()> {
    let sidecar = paths.repo_root().join("src/tui/sidecar.ts");
    if !sidecar.exists() {
        bail!(
            "OpenTUI sidecar is not available at {}; run from a checkout that includes src/tui",
            sidecar.display()
        );
    }
    let opentui_dep = paths.repo_root().join("node_modules/@opentui/core");
    if !opentui_dep.exists() {
        bail!(
            "OpenTUI dependencies are not installed; run `bun install` in {} before using `--renderer opentui`",
            paths.repo_root().display()
        );
    }

    let snapshot_file = temp_snapshot_path();
    fs::write(
        &snapshot_file,
        serde_json::to_vec_pretty(&snapshot(paths)?)?,
    )
    .with_context(|| {
        format!(
            "write temporary Mission Control snapshot {}",
            snapshot_file.display()
        )
    })?;

    let result = run_opentui_child(paths, args, size, &snapshot_file);
    if let Err(error) = fs::remove_file(&snapshot_file)
        && result.is_ok()
    {
        return Err(error).with_context(|| {
            format!(
                "remove temporary Mission Control snapshot {}",
                snapshot_file.display()
            )
        });
    }
    result
}

fn run_opentui_child(
    paths: &MaestroPaths,
    args: &MissionControlArgs,
    size: Option<(usize, usize)>,
    snapshot_file: &Path,
) -> Result<()> {
    let mode = if args.render_check {
        "render-check"
    } else if args.preview.is_some() || args.screen.is_some() {
        "preview"
    } else {
        "interactive"
    };

    let mut command = Command::new("bun");
    command
        .arg("run")
        .arg("src/tui/sidecar.ts")
        .arg("--mode")
        .arg(mode)
        .arg("--snapshot-file")
        .arg(snapshot_file)
        .arg("--cwd")
        .arg(paths.repo_root())
        .current_dir(paths.repo_root())
        .env("MAESTRO_AUTO_UPDATE", "0");

    if let Ok(exe) = std::env::current_exe() {
        command.arg("--maestro-bin").arg(exe);
    }
    if let Some(screen) = raw_screen_arg(args) {
        command.arg("--screen").arg(screen);
    }
    if let Some(feature) = args.feature.as_deref() {
        command.arg("--feature").arg(feature);
    }
    if let Some((width, height)) = size {
        command.arg("--size").arg(format!("{width}x{height}"));
    }
    if let Some(format) = args.format {
        command.arg("--format").arg(format.as_str());
    }

    let status = command
        .status()
        .with_context(|| "run restored TypeScript/OpenTUI Mission Control sidecar")?;
    if !status.success() {
        bail!("OpenTUI Mission Control sidecar exited with {status}");
    }
    Ok(())
}

fn temp_snapshot_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "maestro-mission-control-{}-{nanos}.json",
        std::process::id()
    ))
}

fn raw_screen_arg(args: &MissionControlArgs) -> Option<&str> {
    if let Some(screen) = args.screen.as_deref() {
        return Some(screen);
    }
    match args.preview.as_ref() {
        Some(Some(screen)) => Some(screen.as_str()),
        Some(None) => Some("dashboard"),
        None => None,
    }
}

impl MissionControlFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Ansi => "ansi",
        }
    }
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
