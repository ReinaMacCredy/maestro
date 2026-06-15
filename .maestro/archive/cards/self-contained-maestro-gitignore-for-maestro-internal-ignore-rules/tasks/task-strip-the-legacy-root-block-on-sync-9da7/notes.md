# Strip the legacy root block on sync + clean up on uninstall

2026-06-15  Entry-point correction (see feature notes + ac-7): the legacy-root-block strip is owned by 'maestro install', NOT sync. Title says 'on sync' for the stable slug but the migration lives in install_agent (install-lock-owned). sync/upgrade/init do not migrate. This task = the strip migration in src/domain/install/mod.rs::install_agent_with_writer + uninstall cleanup.
