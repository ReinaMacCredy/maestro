#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "digest"
require "set"

H="/Users/reinamaccredy"; M="#{H}/Code/maestro"
wn=%w[active-ownership-liveness auto-archive-policy automatic-progress-entry automatic-progress-surface autonomous-loop-auto-accept bring-back-mission-control-tui clean-spec-handoff code-review-last-release compact-next-control-plane durable-run-trace harness-engineering-map-for-maestro lean-parity loop-fanout-routing-packet loop-intelligence-layer loop-recipe-execution-packet maestro-project-skill-trigger-descriptions night-backlog plan-before-go-progress-setup rename-ship-to-close repository-harness-ac10-edge-sweep simplify-cli-surface-agents small-task-workflow symphony-loop-improvements verify-edge-sweep-main]
wt=wn.map{|n|"#{M}/.maestro/worktree/#{n}"}
cw=%w[0b86 4796 55d9 5956 7e89 a6db dbff ebad].map{|n|"#{H}/.codex/worktrees/#{n}/maestro"}
lease="#{M}/.worktrees/symphony-work-lease"; cl="#{M}/.claude/worktrees/agent-a941168cbc20f0516"
imp="#{H}/Code/maestro-implicit-ship"; tok="#{H}/Code/maestro-token-diet"; xl="#{H}/Code/maestro-xlink"
bak="#{H}/BackUp/.Code.in-progress-2026-06-16_00-00-04"
roots=[M]+wt+cw+[lease,cl,imp,tok,xl]; nonmain=wt+cw+[lease,cl,imp,tok,xl]
def paths(root)
  return [] unless File.directory?(root)
  Dir.glob("#{root}/**/*",File::FNM_DOTMATCH).select{|p|File.file?(p)||File.symlink?(p)}
end
def present(a) a.select{|p|File.exist?(p)||File.symlink?(p)}.map{|p|File.expand_path(p)}.uniq.sort end
ix=->(n){wt.fetch(wn.index(n))}
legacy={}
a=paths("#{H}/.maestro/skills")+["#{H}/.maestro/skills-lock.yaml"]+paths("#{H}/.maestro/skill-backups")
%W[#{H}/.agents/skills #{H}/.claude/skills #{H}/.codex/skills].each{|d|Dir.glob("#{d}/*",File::FNM_DOTMATCH).each{|p|a<<p if File.symlink?(p)&&(File.realpath(p)rescue "").start_with?("#{H}/.maestro/")}}
legacy[:skill]=present(a)
a=%w[AGENTS.md CLAUDE.md .claude/AGENTS.md .claude/CLAUDE.md .maestro/harness/HARNESS.md .maestro/RECOVERY.md .maestro/harness/harness.yml .maestro/harness/backlog.yaml .maestro/install-lock.yaml .maestro/backups/2026-06-20T19-13-17.728Z-0-update/.maestro/harness/HARNESS.md .maestro/backups/2026-06-20T19-13-17.728Z-0-update/.maestro/RECOVERY.md .maestro/backups/2026-06-21T00-53-17.855Z-0-update/.maestro/harness/HARNESS.md .maestro/backups/2026-06-21T00-53-17.855Z-0-update/.maestro/RECOVERY.md].map{|x|"#{M}/#{x}"}+paths("#{H}/.maestro/harness")
wt.each{|r|%w[AGENTS.md CLAUDE.md .claude/AGENTS.md .claude/CLAUDE.md .maestro/harness/HARNESS.md .maestro/RECOVERY.md .maestro/harness/harness.yml .maestro/harness/backlog.yaml embedded/AGENTS.md embedded/CLAUDE.md embedded/harness/HARNESS.md embedded/harness/RECOVERY.md].each{|x|a<<"#{r}/#{x}"}}
he=ix["harness-engineering-map-for-maestro"]
%w[.maestro/backups/2026-06-20T19-13-17.728Z-0-update/.maestro/harness/HARNESS.md .maestro/backups/2026-06-20T19-13-17.728Z-0-update/.maestro/RECOVERY.md .maestro/backups/2026-06-21T00-53-17.855Z-0-update/.maestro/harness/HARNESS.md .maestro/backups/2026-06-21T00-53-17.855Z-0-update/.maestro/RECOVERY.md .maestro/install-lock.yaml].each{|x|a<<"#{he}/#{x}"}
legacy[:harness]=present(a)
rr=[cw.find{|x|x.include?("/55d9/")},ix["automatic-progress-entry"],ix["automatic-progress-surface"],he,ix["symphony-loop-improvements"],cw.find{|x|x.include?("/4796/")},ix["loop-fanout-routing-packet"],ix["repository-harness-ac10-edge-sweep"],ix["verify-edge-sweep-main"],cw.find{|x|x.include?("/5956/")},ix["compact-next-control-plane"],ix["loop-intelligence-layer"],ix["loop-recipe-execution-packet"],ix["plan-before-go-progress-setup"]]
legacy[:recipe]=present(rr.flat_map{|r|paths("#{r}/embedded/loop-recipes")})
a=nonmain.flat_map{|r|%w[embedded/hooks/events.yaml embedded/hooks/record.sh].map{|x|"#{r}/#{x}"}}+(nonmain+[M]).map{|r|"#{r}/.maestro/hooks/record.sh"}
a+=["#{H}/.maestro/hooks/record.sh","#{M}/.maestro/backups/2026-06-20T19-13-17.728Z-0-update/.maestro/hooks/record.sh","#{he}/.maestro/backups/2026-06-20T19-13-17.728Z-0-update/.maestro/hooks/record.sh","#{M}/.claude/settings.local.json","#{M}/.factory/hooks.json"]
legacy[:hook]=present(a)
runroots=[H,M,tok,cl,ix["code-review-last-release"],ix["durable-run-trace"],he,ix["night-backlog"],ix["simplify-cli-surface-agents"],ix["small-task-workflow"]]
legacy[:run]=present(runroots.flat_map{|r|paths("#{r}/.maestro/runs")}.select{|p|%w[events.jsonl activity.jsonl run_evidence.yaml lean_mode].include?(File.basename(p))||File.basename(p).match?(/\\Afeature-close-suite-.*\\.log\\z/)})
legacy[:schema]=present((cw+wt+[lease,imp,tok,xl,"#{bak}/maestro","#{bak}/maestro-xlink"]).flat_map{|r|paths("#{r}/embedded/schemas")})
legacy[:shell]=present((cw+wt+[lease,cl,imp,tok,xl,"#{bak}/maestro","#{bak}/maestro/.claude/worktrees/agent-a941168cbc20f0516","#{bak}/maestro-xlink"]).flat_map{|r|paths("#{r}/embedded/shell")})
a=(cw+wt+[lease,imp,tok,"#{bak}/maestro"]).flat_map{|r|paths("#{r}/embedded/playbook")}+paths("#{bak}/Technology-News-Collection-and-Summarization-System/.maestro/playbook")
legacy[:playbook]=present(a)
dr=[cw.find{|x|x.include?("/4796/")},cw.find{|x|x.include?("/55d9/")},cw.find{|x|x.include?("/5956/")},ix["automatic-progress-entry"],ix["automatic-progress-surface"],ix["compact-next-control-plane"],he,ix["loop-fanout-routing-packet"],ix["loop-intelligence-layer"],ix["loop-recipe-execution-packet"],ix["plan-before-go-progress-setup"],ix["repository-harness-ac10-edge-sweep"],ix["symphony-loop-improvements"],ix["verify-edge-sweep-main"]]
legacy[:design]=present(dr.flat_map{|r|paths("#{r}/embedded/design")})
hist=wt+cw+[lease,cl,imp,tok]
legacy[:cli]=present(hist.flat_map{|r|Dir.glob("#{r}/embedded/skills/*/reference/cli.md")})
legacy[:readme]=present(hist.flat_map{|r|%w[README.md docs/readme/maestro-card-model.png docs/readme/maestro-cross-agent-coordination.png].map{|x|"#{r}/#{x}"}})
legacy[:mcp_source]=present(roots.flat_map{|r|%w[src/interfaces/cli/mcp.rs src/interfaces/mcp/mod.rs src/interfaces/mcp/server.rs].map{|x|"#{r}/#{x}"}})
a=["#{H}/.claude.json","#{H}/.codex/config.toml","#{H}/.codex/mcp.json"]+Dir.glob("#{H}/.codex/maintenance/backups/*/{config.toml,mcp.json}")+["#{H}/.codex/archived_worktrees/20260622-211233/ac44/maestro/.mcp.json"]
legacy[:mcp_config]=present(a)
m115=<<'LIST'.split
bun.lock
package.json
scripts/tui-dev.ts
src/interfaces/tui/mod.rs
src/shared/errors.ts
src/shared/lib/sanitize.ts
src/tui/AGENTS.md
src/tui/CLAUDE.md
src/tui/app/input-dispatch.ts
src/tui/app/interactive-shared.ts
src/tui/app/modal-builders.ts
src/tui/app/preview-contract.ts
src/tui/app/preview-state.ts
src/tui/app/render-check-contract.ts
src/tui/input.ts
src/tui/opentui/ansi.ts
src/tui/opentui/app/mission-control-app.tsx
src/tui/opentui/app/preview.ts
src/tui/opentui/app/render-check.ts
src/tui/opentui/components/builders.ts
src/tui/opentui/components/mission-control-screen.tsx
src/tui/opentui/index.ts
src/tui/opentui/testing/frame-capture.tsx
src/tui/shared/format.ts
src/tui/shared/header-animation.ts
src/tui/shared/modal-model.ts
src/tui/shared/session-id.ts
src/tui/shared/theme.ts
src/tui/shared/ui-config.ts
src/tui/state/screen-types.ts
src/tui/state/types.ts
tsconfig.json
src/features/evidence/index.ts
src/features/principle/index.ts
src/features/reply/index.ts
src/features/verdict/index.ts
src/infra/domain/config-types.ts
src/infra/domain/git-types.ts
src/infra/domain/status-types.ts
src/infra/ports/config.port.ts
src/infra/ports/git.port.ts
src/infra/usecases/config-edit.usecase.ts
src/interfaces/cli/mission_control.rs
src/interfaces/cli/watch.rs
src/interfaces/tui/task_list_watch.rs
src/repo/contract-store.port.ts
src/repo/run-state-store.port.ts
src/service/contract-helpers.ts
src/shared/domain/legacy-mission.ts
src/shared/domain/task/index.ts
src/tui/opentui/app/interactive.tsx
src/tui/sidecar.ts
src/tui/state/autopilot-screen.ts
src/tui/state/config-inspector.ts
src/tui/state/environment-projection.ts
src/tui/state/events.ts
src/tui/state/memory-projection.ts
src/tui/state/mission-control-commands.ts
src/tui/state/projection.ts
src/tui/state/reducer.ts
src/tui/state/reply-projection.ts
src/tui/state/snapshot-demand.ts
src/tui/state/snapshot-poll-cache.ts
src/tui/state/task-board.ts
src/domain/install/CLAUDE.md
src/domain/run/CLAUDE.md
src/foundation/core/diff.rs
src/foundation/core/hash.rs
ARCHITECTURE.md
CONTRIBUTING.md
Cargo.toml
MAINTENANCE.md
TESTING.md
scripts/install-local.sh
scripts/release-notes.sh
scripts/verify-all.sh
src/domain/extraction/extract.rs
src/domain/install/AGENTS.md
src/domain/run/AGENTS.md
src/foundation/core/backup.rs
src/foundation/core/fs.rs
src/foundation/core/git.rs
src/foundation/core/managed_blocks.rs
src/foundation/core/managed_path.rs
src/foundation/core/safe_write.rs
src/interfaces/cli/init.rs
src/interfaces/cli/install.rs
src/interfaces/cli/sync.rs
src/interfaces/cli/uninstall.rs
src/interfaces/cli/update.rs
src/lib.rs
src/operations/update/github_release.rs
src/operations/update/replace.rs
tests/common/cli_harness.rs
tests/core_backup_diff_git.rs
tests/core_paths_fs.rs
Cargo.lock
scripts/install.sh
src/domain/feature/mod.rs
src/domain/install/mirrors.rs
src/domain/install/mod.rs
src/domain/proof/mod.rs
src/domain/task/mod.rs
src/foundation/core/time.rs
src/interfaces/cli/decision.rs
src/interfaces/cli/qa.rs
src/interfaces/cli/scorer.rs
src/operations/init/mod.rs
src/operations/sync/mod.rs
MIGRATE.md
tests/feature_qa_gate_integration.rs
tests/install_dry_run_integration.rs
tests/local_install_script.rs
tests/sync_integration.rs
src/interfaces/cli/memory.rs
LIST
raise "m115 #{m115.size}" unless m115.size==115
cats={}
cats[:legacy]=legacy.values.flatten.uniq.sort
cats[:c115]=present(roots.flat_map{|r|m115.map{|x|"#{r}/#{x}"}})
names=Set.new(%w[@opentui/core @opentui/react react cli-spinners]);q=names.to_a
until q.empty?
 n=q.shift; p="#{M}/node_modules/#{n}/package.json";next unless File.file?(p)
 j=JSON.parse(File.binread(p));(j.fetch("dependencies",{}).merge(j.fetch("optionalDependencies",{})).merge(j.fetch("peerDependencies",{}))).each_key{|d|if File.file?("#{M}/node_modules/#{d}/package.json")&&!names.include?(d);names<<d;q<<d end}
end
cats[:repo]=present(names.flat_map{|n|paths("#{M}/node_modules/#{n}")})
cr=names.flat_map{|n|s="#{H}/.bun/install/cache/#{n}";[s]+Dir.glob("#{s}@*")}.select{|p|File.exist?(p)||File.symlink?(p)}.uniq.sort
cats[:cache]=present(cr.flat_map{|r|File.symlink?(r)?[r]:paths(r)})
a=roots.flat_map{|r|%w[target/debug/maestro target/release/maestro].map{|x|"#{r}/#{x}"}}+["#{H}/.local/bin/maestro","#{H}/.cargo/bin/maestro","#{H}/.bun/bin/bun","#{H}/.cargo/bin/maestro.v0.1.0.bak","#{H}/.local/bin/maestro-test-copy","#{H}/.local/bin/maestro.pre-rust-20260526-1327","#{H}/.local/bin/maestro.ts-0.106.1.bak"]
cats[:binary]=present(a)
cats[:perroot]=present(roots.flat_map{|r|["#{r}/.maestro/update-check","#{r}/.maestro/global-skills-warning",*Dir.glob("#{r}/.maestro/backups/*-install/.gitignore")]})
cats[:user]=present(paths("#{H}/.maestro/skills.prelock-backup-20260605T1356Z")+paths("#{H}/.maestro/backups/maestro-design-unmanaged-20260702T232932Z")+["#{H}/.maestro/update-check","#{H}/.maestro/update-check.json"])
expected={legacy:4816,c115:3230,repo:4665,cache:15277,binary:64,perroot:34,user:16}
expected.each{|k,n|raise "#{k} #{cats[k].size}" unless cats[k].size==n}
flat=cats.values.flatten;raise "overlap #{flat.size-flat.uniq.size}" unless flat.size==flat.uniq.size
locs=flat.sort
raise "union #{locs.size}" unless locs.size==28102
def take(locs)
 rows=[]; bytes=0; sig={}
 locs.each do |p|
  if File.symlink?(p)
   type="L";payload=File.readlink(p).b
  else
   type="F";payload=File.binread(p)
  end
  len=payload.bytesize;sha=Digest::SHA256.hexdigest(payload);loc=File.expand_path(p)
  row="#{type}\\t#{len}\\t#{sha}\\t#{loc}\\n"
  rows<<row;bytes+=len;sig[loc]=[type,len,sha]
 end
 [Digest::SHA256.hexdigest(rows.join),bytes,sig]
end
locator=Digest::SHA256.hexdigest(locs.map{|p|"#{File.expand_path(p)}\\n"}.join)
a=take(locs);b=take(locs)
changed=a[2].keys.select{|p|a[2][p]!=b[2][p]}
puts "COUNTS #{cats.map{|k,v|"#{k}=#{v.size}"}.join(" ")} total=#{locs.size}"
puts "TYPES files=#{locs.count{|p|!File.symlink?(p)}} symlinks=#{locs.count{|p|File.symlink?(p)}}"
puts "LOCATOR #{locator}"
puts "PASS1 identity=#{a[0]} bytes=#{a[1]}"
puts "PASS2 identity=#{b[0]} bytes=#{b[1]}"
puts "STABLE #{a[0]==b[0]&&a[1]==b[1]&&changed.empty?} changed=#{changed.size}"
changed.each{|p|puts "CHANGED #{p} #{a[2][p].inspect} -> #{b[2][p].inspect}"}
