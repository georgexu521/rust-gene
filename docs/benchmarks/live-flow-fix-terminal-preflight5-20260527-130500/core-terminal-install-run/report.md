# Live Eval Report: core-terminal-install-run

- Run id: `flow-fix-terminal-preflight5-20260527-130500`
- Sample: `evalsets/live_tasks/core-terminal-install-run.yaml`
- Worktree: `target/live-evals/flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/env`
- Test status: `failed`
- Generated: `2026-05-27 13:00:24 +0800`

## Git Status

```text
?? .venv/
```

## Diff Stat

```text
 /dev/null => .venv/bin/Activate.ps1 | 247 ++++++++++++++++++++++++++++++++++++
 1 file changed, 247 insertions(+)
 /dev/null => .venv/bin/activate | 70 +++++++++++++++++++++++++++++++++++++++++
 1 file changed, 70 insertions(+)
 /dev/null => .venv/bin/activate.csh | 27 +++++++++++++++++++++++++++
 1 file changed, 27 insertions(+)
 /dev/null => .venv/bin/activate.fish | 69 ++++++++++++++++++++++++++++++++++++
 1 file changed, 69 insertions(+)
 /dev/null => .venv/bin/pip | 8 ++++++++
 1 file changed, 8 insertions(+)
 /dev/null => .venv/bin/pip3 | 8 ++++++++
 1 file changed, 8 insertions(+)
 /dev/null => .venv/bin/pip3.12 | 8 ++++++++
 1 file changed, 8 insertions(+)
 /dev/null => .venv/bin/python | 1 +
 1 file changed, 1 insertion(+)
 /dev/null => .venv/bin/python3 | 1 +
 1 file changed, 1 insertion(+)
 /dev/null => .venv/bin/python3.12 | 1 +
 1 file changed, 1 insertion(+)
 .../site-packages/pip-24.0.dist-info/AUTHORS.txt   | 760 +++++++++++++++++++++
 1 file changed, 760 insertions(+)
 .../lib/python3.12/site-packages/pip-24.0.dist-info/INSTALLER            | 1 +
 1 file changed, 1 insertion(+)
 .../site-packages/pip-24.0.dist-info/LICENSE.txt     | 20 ++++++++++++++++++++
 1 file changed, 20 insertions(+)
 .../site-packages/pip-24.0.dist-info/METADATA      | 88 ++++++++++++++++++++++
 1 file changed, 88 insertions(+)
 .../site-packages/pip-24.0.dist-info/RECORD        | 1024 ++++++++++++++++++++
 1 file changed, 1024 insertions(+)
 .../lib/python3.12/site-packages/pip-24.0.dist-info/REQUESTED             | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../lib/python3.12/site-packages/pip-24.0.dist-info/WHEEL            | 5 +++++
 1 file changed, 5 insertions(+)
 .../lib/python3.12/site-packages/pip-24.0.dist-info/entry_points.txt  | 4 ++++
 1 file changed, 4 insertions(+)
 .../lib/python3.12/site-packages/pip-24.0.dist-info/top_level.txt        | 1 +
 1 file changed, 1 insertion(+)
 .../lib/python3.12/site-packages/pip/__init__.py            | 13 +++++++++++++
 1 file changed, 13 insertions(+)
 .../lib/python3.12/site-packages/pip/__main__.py   | 24 ++++++++++++++++++++++
 1 file changed, 24 insertions(+)
 .../python3.12/site-packages/pip/__pip-runner__.py | 50 ++++++++++++++++++++++
 1 file changed, 50 insertions(+)
 .../python3.12/site-packages/pip/_internal/__init__.py | 18 ++++++++++++++++++
 1 file changed, 18 insertions(+)
 .../site-packages/pip/_internal/build_env.py       | 311 +++++++++++++++++++++
 1 file changed, 311 insertions(+)
 .../site-packages/pip/_internal/cache.py           | 290 +++++++++++++++++++++
 1 file changed, 290 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/cli/__init__.py        | 4 ++++
 1 file changed, 4 insertions(+)
 .../pip/_internal/cli/autocompletion.py            | 172 +++++++++++++++++++++
 1 file changed, 172 insertions(+)
 .../pip/_internal/cli/base_command.py              | 236 +++++++++++++++++++++
 1 file changed, 236 insertions(+)
 .../site-packages/pip/_internal/cli/cmdoptions.py  | 1074 ++++++++++++++++++++
 1 file changed, 1074 insertions(+)
 .../pip/_internal/cli/command_context.py           | 27 ++++++++++++++++++++++
 1 file changed, 27 insertions(+)
 .../site-packages/pip/_internal/cli/main.py        | 79 ++++++++++++++++++++++
 1 file changed, 79 insertions(+)
 .../site-packages/pip/_internal/cli/main_parser.py | 134 +++++++++++++++++++++
 1 file changed, 134 insertions(+)
 .../site-packages/pip/_internal/cli/parser.py      | 294 +++++++++++++++++++++
 1 file changed, 294 insertions(+)
 .../pip/_internal/cli/progress_bars.py             | 68 ++++++++++++++++++++++
 1 file changed, 68 insertions(+)
 .../site-packages/pip/_internal/cli/req_command.py | 505 +++++++++++++++++++++
 1 file changed, 505 insertions(+)
 .../site-packages/pip/_internal/cli/spinners.py    | 159 +++++++++++++++++++++
 1 file changed, 159 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/cli/status_codes.py  | 6 ++++++
 1 file changed, 6 insertions(+)
 .../pip/_internal/commands/__init__.py             | 132 +++++++++++++++++++++
 1 file changed, 132 insertions(+)
 .../site-packages/pip/_internal/commands/cache.py  | 225 +++++++++++++++++++++
 1 file changed, 225 insertions(+)
 .../site-packages/pip/_internal/commands/check.py  | 54 ++++++++++++++++++++++
 1 file changed, 54 insertions(+)
 .../pip/_internal/commands/completion.py           | 130 +++++++++++++++++++++
 1 file changed, 130 insertions(+)
 .../pip/_internal/commands/configuration.py        | 280 +++++++++++++++++++++
 1 file changed, 280 insertions(+)
 .../site-packages/pip/_internal/commands/debug.py  | 201 +++++++++++++++++++++
 1 file changed, 201 insertions(+)
 .../pip/_internal/commands/download.py             | 147 +++++++++++++++++++++
 1 file changed, 147 insertions(+)
 .../site-packages/pip/_internal/commands/freeze.py | 108 +++++++++++++++++++++
 1 file changed, 108 insertions(+)
 .../site-packages/pip/_internal/commands/hash.py   | 59 ++++++++++++++++++++++
 1 file changed, 59 insertions(+)
 .../site-packages/pip/_internal/commands/help.py   | 41 ++++++++++++++++++++++
 1 file changed, 41 insertions(+)
 .../site-packages/pip/_internal/commands/index.py  | 139 +++++++++++++++++++++
 1 file changed, 139 insertions(+)
 .../pip/_internal/commands/inspect.py              | 92 ++++++++++++++++++++++
 1 file changed, 92 insertions(+)
 .../pip/_internal/commands/install.py              | 774 +++++++++++++++++++++
 1 file changed, 774 insertions(+)
 .../site-packages/pip/_internal/commands/list.py   | 368 +++++++++++++++++++++
 1 file changed, 368 insertions(+)
 .../site-packages/pip/_internal/commands/search.py | 174 +++++++++++++++++++++
 1 file changed, 174 insertions(+)
 .../site-packages/pip/_internal/commands/show.py   | 189 +++++++++++++++++++++
 1 file changed, 189 insertions(+)
 .../pip/_internal/commands/uninstall.py            | 113 +++++++++++++++++++++
 1 file changed, 113 insertions(+)
 .../site-packages/pip/_internal/commands/wheel.py  | 183 +++++++++++++++++++++
 1 file changed, 183 insertions(+)
 .../site-packages/pip/_internal/configuration.py   | 383 +++++++++++++++++++++
 1 file changed, 383 insertions(+)
 .../pip/_internal/distributions/__init__.py         | 21 +++++++++++++++++++++
 1 file changed, 21 insertions(+)
 .../pip/_internal/distributions/base.py            | 51 ++++++++++++++++++++++
 1 file changed, 51 insertions(+)
 .../pip/_internal/distributions/installed.py       | 29 ++++++++++++++++++++++
 1 file changed, 29 insertions(+)
 .../pip/_internal/distributions/sdist.py           | 156 +++++++++++++++++++++
 1 file changed, 156 insertions(+)
 .../pip/_internal/distributions/wheel.py           | 40 ++++++++++++++++++++++
 1 file changed, 40 insertions(+)
 .../site-packages/pip/_internal/exceptions.py      | 728 +++++++++++++++++++++
 1 file changed, 728 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/index/__init__.py        | 2 ++
 1 file changed, 2 insertions(+)
 .../site-packages/pip/_internal/index/collector.py | 507 +++++++++++++++++++++
 1 file changed, 507 insertions(+)
 .../pip/_internal/index/package_finder.py          | 1027 ++++++++++++++++++++
 1 file changed, 1027 insertions(+)
 .../site-packages/pip/_internal/index/sources.py   | 285 +++++++++++++++++++++
 1 file changed, 285 insertions(+)
 .../pip/_internal/locations/__init__.py            | 467 +++++++++++++++++++++
 1 file changed, 467 insertions(+)
 .../pip/_internal/locations/_distutils.py          | 172 +++++++++++++++++++++
 1 file changed, 172 insertions(+)
 .../pip/_internal/locations/_sysconfig.py          | 213 +++++++++++++++++++++
 1 file changed, 213 insertions(+)
 .../site-packages/pip/_internal/locations/base.py  | 81 ++++++++++++++++++++++
 1 file changed, 81 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/main.py       | 12 ++++++++++++
 1 file changed, 12 insertions(+)
 .../pip/_internal/metadata/__init__.py             | 128 +++++++++++++++++++++
 1 file changed, 128 insertions(+)
 .../site-packages/pip/_internal/metadata/_json.py  | 84 ++++++++++++++++++++++
 1 file changed, 84 insertions(+)
 .../site-packages/pip/_internal/metadata/base.py   | 702 +++++++++++++++++++++
 1 file changed, 702 insertions(+)
 .../site-packages/pip/_internal/metadata/importlib/__init__.py      | 6 ++++++
 1 file changed, 6 insertions(+)
 .../pip/_internal/metadata/importlib/_compat.py    | 55 ++++++++++++++++++++++
 1 file changed, 55 insertions(+)
 .../pip/_internal/metadata/importlib/_dists.py     | 227 +++++++++++++++++++++
 1 file changed, 227 insertions(+)
 .../pip/_internal/metadata/importlib/_envs.py      | 189 +++++++++++++++++++++
 1 file changed, 189 insertions(+)
 .../pip/_internal/metadata/pkg_resources.py        | 278 +++++++++++++++++++++
 1 file changed, 278 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/models/__init__.py       | 2 ++
 1 file changed, 2 insertions(+)
 .../pip/_internal/models/candidate.py              | 30 ++++++++++++++++++++++
 1 file changed, 30 insertions(+)
 .../pip/_internal/models/direct_url.py             | 235 +++++++++++++++++++++
 1 file changed, 235 insertions(+)
 .../pip/_internal/models/format_control.py         | 78 ++++++++++++++++++++++
 1 file changed, 78 insertions(+)
 .../site-packages/pip/_internal/models/index.py    | 28 ++++++++++++++++++++++
 1 file changed, 28 insertions(+)
 .../pip/_internal/models/installation_report.py    | 56 ++++++++++++++++++++++
 1 file changed, 56 insertions(+)
 .../site-packages/pip/_internal/models/link.py     | 579 +++++++++++++++++++++
 1 file changed, 579 insertions(+)
 .../site-packages/pip/_internal/models/scheme.py   | 31 ++++++++++++++++++++++
 1 file changed, 31 insertions(+)
 .../pip/_internal/models/search_scope.py           | 132 +++++++++++++++++++++
 1 file changed, 132 insertions(+)
 .../pip/_internal/models/selection_prefs.py        | 51 ++++++++++++++++++++++
 1 file changed, 51 insertions(+)
 .../pip/_internal/models/target_python.py          | 122 +++++++++++++++++++++
 1 file changed, 122 insertions(+)
 .../site-packages/pip/_internal/models/wheel.py    | 92 ++++++++++++++++++++++
 1 file changed, 92 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/network/__init__.py      | 2 ++
 1 file changed, 2 insertions(+)
 .../site-packages/pip/_internal/network/auth.py    | 561 +++++++++++++++++++++
 1 file changed, 561 insertions(+)
 .../site-packages/pip/_internal/network/cache.py   | 106 +++++++++++++++++++++
 1 file changed, 106 insertions(+)
 .../pip/_internal/network/download.py              | 186 +++++++++++++++++++++
 1 file changed, 186 insertions(+)
 .../pip/_internal/network/lazy_wheel.py            | 210 +++++++++++++++++++++
 1 file changed, 210 insertions(+)
 .../site-packages/pip/_internal/network/session.py | 520 +++++++++++++++++++++
 1 file changed, 520 insertions(+)
 .../site-packages/pip/_internal/network/utils.py   | 96 ++++++++++++++++++++++
 1 file changed, 96 insertions(+)
 .../site-packages/pip/_internal/network/xmlrpc.py  | 62 ++++++++++++++++++++++
 1 file changed, 62 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/operations/__init__.py     | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../python3.12/site-packages/pip/_internal/operations/build/__init__.py   | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../_internal/operations/build/build_tracker.py    | 139 +++++++++++++++++++++
 1 file changed, 139 insertions(+)
 .../pip/_internal/operations/build/metadata.py     | 39 ++++++++++++++++++++++
 1 file changed, 39 insertions(+)
 .../operations/build/metadata_editable.py          | 41 ++++++++++++++++++++++
 1 file changed, 41 insertions(+)
 .../_internal/operations/build/metadata_legacy.py  | 74 ++++++++++++++++++++++
 1 file changed, 74 insertions(+)
 .../pip/_internal/operations/build/wheel.py        | 37 ++++++++++++++++++++++
 1 file changed, 37 insertions(+)
 .../_internal/operations/build/wheel_editable.py   | 46 ++++++++++++++++++++++
 1 file changed, 46 insertions(+)
 .../pip/_internal/operations/build/wheel_legacy.py | 102 +++++++++++++++++++++
 1 file changed, 102 insertions(+)
 .../pip/_internal/operations/check.py              | 187 +++++++++++++++++++++
 1 file changed, 187 insertions(+)
 .../pip/_internal/operations/freeze.py             | 255 +++++++++++++++++++++
 1 file changed, 255 insertions(+)
 .../site-packages/pip/_internal/operations/install/__init__.py          | 2 ++
 1 file changed, 2 insertions(+)
 .../operations/install/editable_legacy.py          | 46 ++++++++++++++++++++++
 1 file changed, 46 insertions(+)
 .../pip/_internal/operations/install/wheel.py      | 734 +++++++++++++++++++++
 1 file changed, 734 insertions(+)
 .../pip/_internal/operations/prepare.py            | 730 +++++++++++++++++++++
 1 file changed, 730 insertions(+)
 .../site-packages/pip/_internal/pyproject.py       | 179 +++++++++++++++++++++
 1 file changed, 179 insertions(+)
 .../site-packages/pip/_internal/req/__init__.py    | 92 ++++++++++++++++++++++
 1 file changed, 92 insertions(+)
 .../pip/_internal/req/constructors.py              | 576 +++++++++++++++++++++
 1 file changed, 576 insertions(+)
 .../site-packages/pip/_internal/req/req_file.py    | 554 +++++++++++++++++++++
 1 file changed, 554 insertions(+)
 .../site-packages/pip/_internal/req/req_install.py | 923 +++++++++++++++++++++
 1 file changed, 923 insertions(+)
 .../site-packages/pip/_internal/req/req_set.py     | 119 +++++++++++++++++++++
 1 file changed, 119 insertions(+)
 .../pip/_internal/req/req_uninstall.py             | 649 +++++++++++++++++++++
 1 file changed, 649 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/resolution/__init__.py     | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_internal/resolution/base.py   | 20 ++++++++++++++++++++
 1 file changed, 20 insertions(+)
 .../python3.12/site-packages/pip/_internal/resolution/legacy/__init__.py  | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_internal/resolution/legacy/resolver.py    | 598 +++++++++++++++++++++
 1 file changed, 598 insertions(+)
 .../site-packages/pip/_internal/resolution/resolvelib/__init__.py         | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_internal/resolution/resolvelib/base.py    | 141 +++++++++++++++++++++
 1 file changed, 141 insertions(+)
 .../_internal/resolution/resolvelib/candidates.py  | 597 +++++++++++++++++++++
 1 file changed, 597 insertions(+)
 .../pip/_internal/resolution/resolvelib/factory.py | 812 +++++++++++++++++++++
 1 file changed, 812 insertions(+)
 .../resolution/resolvelib/found_candidates.py      | 155 +++++++++++++++++++++
 1 file changed, 155 insertions(+)
 .../_internal/resolution/resolvelib/provider.py    | 255 +++++++++++++++++++++
 1 file changed, 255 insertions(+)
 .../_internal/resolution/resolvelib/reporter.py    | 80 ++++++++++++++++++++++
 1 file changed, 80 insertions(+)
 .../resolution/resolvelib/requirements.py          | 166 +++++++++++++++++++++
 1 file changed, 166 insertions(+)
 .../_internal/resolution/resolvelib/resolver.py    | 317 +++++++++++++++++++++
 1 file changed, 317 insertions(+)
 .../pip/_internal/self_outdated_check.py           | 248 +++++++++++++++++++++
 1 file changed, 248 insertions(+)
 .../lib/python3.12/site-packages/pip/_internal/utils/__init__.py          | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_internal/utils/_jaraco_text.py            | 109 +++++++++++++++++++++
 1 file changed, 109 insertions(+)
 .../site-packages/pip/_internal/utils/_log.py      | 38 ++++++++++++++++++++++
 1 file changed, 38 insertions(+)
 .../site-packages/pip/_internal/utils/appdirs.py   | 52 ++++++++++++++++++++++
 1 file changed, 52 insertions(+)
 .../site-packages/pip/_internal/utils/compat.py    | 63 ++++++++++++++++++++++
 1 file changed, 63 insertions(+)
 .../pip/_internal/utils/compatibility_tags.py      | 165 +++++++++++++++++++++
 1 file changed, 165 insertions(+)
 .../python3.12/site-packages/pip/_internal/utils/datetime.py  | 11 +++++++++++
 1 file changed, 11 insertions(+)
 .../pip/_internal/utils/deprecation.py             | 120 +++++++++++++++++++++
 1 file changed, 120 insertions(+)
 .../pip/_internal/utils/direct_url_helpers.py      | 87 ++++++++++++++++++++++
 1 file changed, 87 insertions(+)
 .../site-packages/pip/_internal/utils/egg_link.py  | 80 ++++++++++++++++++++++
 1 file changed, 80 insertions(+)
 .../site-packages/pip/_internal/utils/encoding.py  | 36 ++++++++++++++++++++++
 1 file changed, 36 insertions(+)
 .../pip/_internal/utils/entrypoints.py             | 84 ++++++++++++++++++++++
 1 file changed, 84 insertions(+)
 .../pip/_internal/utils/filesystem.py              | 153 +++++++++++++++++++++
 1 file changed, 153 insertions(+)
 .../site-packages/pip/_internal/utils/filetypes.py | 27 ++++++++++++++++++++++
 1 file changed, 27 insertions(+)
 .../site-packages/pip/_internal/utils/glibc.py     | 88 ++++++++++++++++++++++
 1 file changed, 88 insertions(+)
 .../site-packages/pip/_internal/utils/hashes.py    | 151 +++++++++++++++++++++
 1 file changed, 151 insertions(+)
 .../site-packages/pip/_internal/utils/logging.py   | 348 +++++++++++++++++++++
 1 file changed, 348 insertions(+)
 .../site-packages/pip/_internal/utils/misc.py      | 783 +++++++++++++++++++++
 1 file changed, 783 insertions(+)
 .../site-packages/pip/_internal/utils/models.py    | 39 ++++++++++++++++++++++
 1 file changed, 39 insertions(+)
 .../site-packages/pip/_internal/utils/packaging.py | 57 ++++++++++++++++++++++
 1 file changed, 57 insertions(+)
 .../pip/_internal/utils/setuptools_build.py        | 146 +++++++++++++++++++++
 1 file changed, 146 insertions(+)
 .../pip/_internal/utils/subprocess.py              | 260 +++++++++++++++++++++
 1 file changed, 260 insertions(+)
 .../site-packages/pip/_internal/utils/temp_dir.py  | 296 +++++++++++++++++++++
 1 file changed, 296 insertions(+)
 .../site-packages/pip/_internal/utils/unpacking.py | 257 +++++++++++++++++++++
 1 file changed, 257 insertions(+)
 .../site-packages/pip/_internal/utils/urls.py      | 62 ++++++++++++++++++++++
 1 file changed, 62 insertions(+)
 .../pip/_internal/utils/virtualenv.py              | 104 +++++++++++++++++++++
 1 file changed, 104 insertions(+)
 .../site-packages/pip/_internal/utils/wheel.py     | 134 +++++++++++++++++++++
 1 file changed, 134 insertions(+)
 .../site-packages/pip/_internal/vcs/__init__.py           | 15 +++++++++++++++
 1 file changed, 15 insertions(+)
 .../site-packages/pip/_internal/vcs/bazaar.py      | 112 +++++++++++++++++++++
 1 file changed, 112 insertions(+)
 .../site-packages/pip/_internal/vcs/git.py         | 526 +++++++++++++++++++++
 1 file changed, 526 insertions(+)
 .../site-packages/pip/_internal/vcs/mercurial.py   | 163 +++++++++++++++++++++
 1 file changed, 163 insertions(+)
 .../site-packages/pip/_internal/vcs/subversion.py  | 324 +++++++++++++++++++++
 1 file changed, 324 insertions(+)
 .../pip/_internal/vcs/versioncontrol.py            | 705 +++++++++++++++++++++
 1 file changed, 705 insertions(+)
 .../site-packages/pip/_internal/wheel_builder.py   | 354 +++++++++++++++++++++
 1 file changed, 354 insertions(+)
 .../site-packages/pip/_vendor/__init__.py          | 121 +++++++++++++++++++++
 1 file changed, 121 insertions(+)
 .../pip/_vendor/cachecontrol/__init__.py           | 28 ++++++++++++++++++++++
 1 file changed, 28 insertions(+)
 .../site-packages/pip/_vendor/cachecontrol/_cmd.py | 70 ++++++++++++++++++++++
 1 file changed, 70 insertions(+)
 .../pip/_vendor/cachecontrol/adapter.py            | 161 +++++++++++++++++++++
 1 file changed, 161 insertions(+)
 .../pip/_vendor/cachecontrol/cache.py              | 74 ++++++++++++++++++++++
 1 file changed, 74 insertions(+)
 .../site-packages/pip/_vendor/cachecontrol/caches/__init__.py     | 8 ++++++++
 1 file changed, 8 insertions(+)
 .../pip/_vendor/cachecontrol/caches/file_cache.py  | 181 +++++++++++++++++++++
 1 file changed, 181 insertions(+)
 .../pip/_vendor/cachecontrol/caches/redis_cache.py | 48 ++++++++++++++++++++++
 1 file changed, 48 insertions(+)
 .../pip/_vendor/cachecontrol/controller.py         | 494 +++++++++++++++++++++
 1 file changed, 494 insertions(+)
 .../pip/_vendor/cachecontrol/filewrapper.py        | 119 +++++++++++++++++++++
 1 file changed, 119 insertions(+)
 .../pip/_vendor/cachecontrol/heuristics.py         | 154 +++++++++++++++++++++
 1 file changed, 154 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/cachecontrol/py.typed        | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_vendor/cachecontrol/serialize.py          | 206 +++++++++++++++++++++
 1 file changed, 206 insertions(+)
 .../pip/_vendor/cachecontrol/wrapper.py            | 43 ++++++++++++++++++++++
 1 file changed, 43 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/certifi/__init__.py      | 4 ++++
 1 file changed, 4 insertions(+)
 .../python3.12/site-packages/pip/_vendor/certifi/__main__.py | 12 ++++++++++++
 1 file changed, 12 insertions(+)
 .../site-packages/pip/_vendor/certifi/cacert.pem   | 4635 ++++++++++++++++++++
 1 file changed, 4635 insertions(+)
 .../site-packages/pip/_vendor/certifi/core.py      | 108 +++++++++++++++++++++
 1 file changed, 108 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/certifi/py.typed             | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/chardet/__init__.py  | 115 +++++++++++++++++++++
 1 file changed, 115 insertions(+)
 .../site-packages/pip/_vendor/chardet/big5freq.py  | 386 +++++++++++++++++++++
 1 file changed, 386 insertions(+)
 .../pip/_vendor/chardet/big5prober.py              | 47 ++++++++++++++++++++++
 1 file changed, 47 insertions(+)
 .../pip/_vendor/chardet/chardistribution.py        | 261 +++++++++++++++++++++
 1 file changed, 261 insertions(+)
 .../pip/_vendor/chardet/charsetgroupprober.py      | 106 +++++++++++++++++++++
 1 file changed, 106 insertions(+)
 .../pip/_vendor/chardet/charsetprober.py           | 147 +++++++++++++++++++++
 1 file changed, 147 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/chardet/cli/__init__.py      | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_vendor/chardet/cli/chardetect.py          | 112 +++++++++++++++++++++
 1 file changed, 112 insertions(+)
 .../pip/_vendor/chardet/codingstatemachine.py      | 90 ++++++++++++++++++++++
 1 file changed, 90 insertions(+)
 .../pip/_vendor/chardet/codingstatemachinedict.py     | 19 +++++++++++++++++++
 1 file changed, 19 insertions(+)
 .../pip/_vendor/chardet/cp949prober.py             | 49 ++++++++++++++++++++++
 1 file changed, 49 insertions(+)
 .../site-packages/pip/_vendor/chardet/enums.py     | 85 ++++++++++++++++++++++
 1 file changed, 85 insertions(+)
 .../site-packages/pip/_vendor/chardet/escprober.py | 102 +++++++++++++++++++++
 1 file changed, 102 insertions(+)
 .../site-packages/pip/_vendor/chardet/escsm.py     | 261 +++++++++++++++++++++
 1 file changed, 261 insertions(+)
 .../pip/_vendor/chardet/eucjpprober.py             | 102 +++++++++++++++++++++
 1 file changed, 102 insertions(+)
 .../site-packages/pip/_vendor/chardet/euckrfreq.py | 196 +++++++++++++++++++++
 1 file changed, 196 insertions(+)
 .../pip/_vendor/chardet/euckrprober.py             | 47 ++++++++++++++++++++++
 1 file changed, 47 insertions(+)
 .../site-packages/pip/_vendor/chardet/euctwfreq.py | 388 +++++++++++++++++++++
 1 file changed, 388 insertions(+)
 .../pip/_vendor/chardet/euctwprober.py             | 47 ++++++++++++++++++++++
 1 file changed, 47 insertions(+)
 .../pip/_vendor/chardet/gb2312freq.py              | 284 +++++++++++++++++++++
 1 file changed, 284 insertions(+)
 .../pip/_vendor/chardet/gb2312prober.py            | 47 ++++++++++++++++++++++
 1 file changed, 47 insertions(+)
 .../pip/_vendor/chardet/hebrewprober.py            | 316 +++++++++++++++++++++
 1 file changed, 316 insertions(+)
 .../site-packages/pip/_vendor/chardet/jisfreq.py   | 325 +++++++++++++++++++++
 1 file changed, 325 insertions(+)
 .../site-packages/pip/_vendor/chardet/johabfreq.py | 2382 ++++++++++++++++++++
 1 file changed, 2382 insertions(+)
 .../pip/_vendor/chardet/johabprober.py             | 47 ++++++++++++++++++++++
 1 file changed, 47 insertions(+)
 .../site-packages/pip/_vendor/chardet/jpcntx.py    | 238 +++++++++++++++++++++
 1 file changed, 238 insertions(+)
 .../pip/_vendor/chardet/langbulgarianmodel.py      | 4649 ++++++++++++++++++++
 1 file changed, 4649 insertions(+)
 .../pip/_vendor/chardet/langgreekmodel.py          | 4397 ++++++++++++++++++++
 1 file changed, 4397 insertions(+)
 .../pip/_vendor/chardet/langhebrewmodel.py         | 4380 ++++++++++++++++++++
 1 file changed, 4380 insertions(+)
 .../pip/_vendor/chardet/langhungarianmodel.py      | 4649 ++++++++++++++++++++
 1 file changed, 4649 insertions(+)
 .../pip/_vendor/chardet/langrussianmodel.py        | 5725 ++++++++++++++++++++
 1 file changed, 5725 insertions(+)
 .../pip/_vendor/chardet/langthaimodel.py           | 4380 ++++++++++++++++++++
 1 file changed, 4380 insertions(+)
 .../pip/_vendor/chardet/langturkishmodel.py        | 4380 ++++++++++++++++++++
 1 file changed, 4380 insertions(+)
 .../pip/_vendor/chardet/latin1prober.py            | 147 +++++++++++++++++++++
 1 file changed, 147 insertions(+)
 .../pip/_vendor/chardet/macromanprober.py          | 162 +++++++++++++++++++++
 1 file changed, 162 insertions(+)
 .../pip/_vendor/chardet/mbcharsetprober.py         | 95 ++++++++++++++++++++++
 1 file changed, 95 insertions(+)
 .../pip/_vendor/chardet/mbcsgroupprober.py         | 57 ++++++++++++++++++++++
 1 file changed, 57 insertions(+)
 .../site-packages/pip/_vendor/chardet/mbcssm.py    | 661 +++++++++++++++++++++
 1 file changed, 661 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/chardet/metadata/__init__.py | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_vendor/chardet/metadata/languages.py      | 352 +++++++++++++++++++++
 1 file changed, 352 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/chardet/py.typed             | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/chardet/resultdict.py      | 16 ++++++++++++++++
 1 file changed, 16 insertions(+)
 .../pip/_vendor/chardet/sbcharsetprober.py         | 162 +++++++++++++++++++++
 1 file changed, 162 insertions(+)
 .../pip/_vendor/chardet/sbcsgroupprober.py         | 88 ++++++++++++++++++++++
 1 file changed, 88 insertions(+)
 .../pip/_vendor/chardet/sjisprober.py              | 105 +++++++++++++++++++++
 1 file changed, 105 insertions(+)
 .../pip/_vendor/chardet/universaldetector.py       | 362 +++++++++++++++++++++
 1 file changed, 362 insertions(+)
 .../pip/_vendor/chardet/utf1632prober.py           | 225 +++++++++++++++++++++
 1 file changed, 225 insertions(+)
 .../pip/_vendor/chardet/utf8prober.py              | 82 ++++++++++++++++++++++
 1 file changed, 82 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/chardet/version.py  | 9 +++++++++
 1 file changed, 9 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/colorama/__init__.py  | 7 +++++++
 1 file changed, 7 insertions(+)
 .../site-packages/pip/_vendor/colorama/ansi.py     | 102 +++++++++++++++++++++
 1 file changed, 102 insertions(+)
 .../pip/_vendor/colorama/ansitowin32.py            | 277 +++++++++++++++++++++
 1 file changed, 277 insertions(+)
 .../pip/_vendor/colorama/initialise.py             | 121 +++++++++++++++++++++
 1 file changed, 121 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/colorama/tests/__init__.py  | 1 +
 1 file changed, 1 insertion(+)
 .../pip/_vendor/colorama/tests/ansi_test.py        | 76 ++++++++++++++++++++++
 1 file changed, 76 insertions(+)
 .../pip/_vendor/colorama/tests/ansitowin32_test.py | 294 +++++++++++++++++++++
 1 file changed, 294 insertions(+)
 .../pip/_vendor/colorama/tests/initialise_test.py  | 189 +++++++++++++++++++++
 1 file changed, 189 insertions(+)
 .../pip/_vendor/colorama/tests/isatty_test.py      | 57 ++++++++++++++++++++++
 1 file changed, 57 insertions(+)
 .../pip/_vendor/colorama/tests/utils.py            | 49 ++++++++++++++++++++++
 1 file changed, 49 insertions(+)
 .../pip/_vendor/colorama/tests/winterm_test.py     | 131 +++++++++++++++++++++
 1 file changed, 131 insertions(+)
 .../site-packages/pip/_vendor/colorama/win32.py    | 180 +++++++++++++++++++++
 1 file changed, 180 insertions(+)
 .../site-packages/pip/_vendor/colorama/winterm.py  | 195 +++++++++++++++++++++
 1 file changed, 195 insertions(+)
 .../site-packages/pip/_vendor/distlib/__init__.py  | 33 ++++++++++++++++++++++
 1 file changed, 33 insertions(+)
 .../site-packages/pip/_vendor/distlib/compat.py    | 1138 ++++++++++++++++++++
 1 file changed, 1138 insertions(+)
 .../site-packages/pip/_vendor/distlib/database.py  | 1359 ++++++++++++++++++++
 1 file changed, 1359 insertions(+)
 .../site-packages/pip/_vendor/distlib/index.py     | 508 +++++++++++++++++++++
 1 file changed, 508 insertions(+)
 .../site-packages/pip/_vendor/distlib/locators.py  | 1303 ++++++++++++++++++++
 1 file changed, 1303 insertions(+)
 .../site-packages/pip/_vendor/distlib/manifest.py  | 384 +++++++++++++++++++++
 1 file changed, 384 insertions(+)
 .../site-packages/pip/_vendor/distlib/markers.py   | 167 +++++++++++++++++++++
 1 file changed, 167 insertions(+)
 .../site-packages/pip/_vendor/distlib/metadata.py  | 1068 ++++++++++++++++++++
 1 file changed, 1068 insertions(+)
 .../site-packages/pip/_vendor/distlib/resources.py | 358 +++++++++++++++++++++
 1 file changed, 358 insertions(+)
 .../site-packages/pip/_vendor/distlib/scripts.py   | 452 +++++++++++++++++++++
 1 file changed, 452 insertions(+)
 .../site-packages/pip/_vendor/distlib/t32.exe           | Bin 0 -> 97792 bytes
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/distlib/t64-arm.exe      | Bin 0 -> 182784 bytes
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/distlib/t64.exe          | Bin 0 -> 108032 bytes
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/distlib/util.py      | 2025 ++++++++++++++++++++
 1 file changed, 2025 insertions(+)
 .../site-packages/pip/_vendor/distlib/version.py   | 751 +++++++++++++++++++++
 1 file changed, 751 insertions(+)
 .../site-packages/pip/_vendor/distlib/w32.exe           | Bin 0 -> 91648 bytes
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/distlib/w64-arm.exe      | Bin 0 -> 168448 bytes
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/distlib/w64.exe          | Bin 0 -> 101888 bytes
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/distlib/wheel.py     | 1099 ++++++++++++++++++++
 1 file changed, 1099 insertions(+)
 .../site-packages/pip/_vendor/distro/__init__.py   | 54 ++++++++++++++++++++++
 1 file changed, 54 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/distro/__main__.py       | 4 ++++
 1 file changed, 4 insertions(+)
 .../site-packages/pip/_vendor/distro/distro.py     | 1399 ++++++++++++++++++++
 1 file changed, 1399 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/distro/py.typed              | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/idna/__init__.py     | 44 ++++++++++++++++++++++
 1 file changed, 44 insertions(+)
 .../site-packages/pip/_vendor/idna/codec.py        | 112 +++++++++++++++++++++
 1 file changed, 112 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/idna/compat.py | 13 +++++++++++++
 1 file changed, 13 insertions(+)
 .../site-packages/pip/_vendor/idna/core.py         | 400 +++++++++++++++++++++
 1 file changed, 400 insertions(+)
 .../site-packages/pip/_vendor/idna/idnadata.py     | 2151 ++++++++++++++++++++
 1 file changed, 2151 insertions(+)
 .../site-packages/pip/_vendor/idna/intranges.py    | 54 ++++++++++++++++++++++
 1 file changed, 54 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/idna/package_data.py       | 2 ++
 1 file changed, 2 insertions(+)
 /dev/null => .venv/lib/python3.12/site-packages/pip/_vendor/idna/py.typed | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/idna/uts46data.py    | 8600 ++++++++++++++++++++
 1 file changed, 8600 insertions(+)
 .../site-packages/pip/_vendor/msgpack/__init__.py  | 57 ++++++++++++++++++++++
 1 file changed, 57 insertions(+)
 .../pip/_vendor/msgpack/exceptions.py              | 48 ++++++++++++++++++++++
 1 file changed, 48 insertions(+)
 .../site-packages/pip/_vendor/msgpack/ext.py       | 193 +++++++++++++++++++++
 1 file changed, 193 insertions(+)
 .../site-packages/pip/_vendor/msgpack/fallback.py  | 1010 ++++++++++++++++++++
 1 file changed, 1010 insertions(+)
 .../pip/_vendor/packaging/__about__.py             | 26 ++++++++++++++++++++++
 1 file changed, 26 insertions(+)
 .../pip/_vendor/packaging/__init__.py              | 25 ++++++++++++++++++++++
 1 file changed, 25 insertions(+)
 .../pip/_vendor/packaging/_manylinux.py            | 301 +++++++++++++++++++++
 1 file changed, 301 insertions(+)
 .../pip/_vendor/packaging/_musllinux.py            | 136 +++++++++++++++++++++
 1 file changed, 136 insertions(+)
 .../pip/_vendor/packaging/_structures.py           | 61 ++++++++++++++++++++++
 1 file changed, 61 insertions(+)
 .../site-packages/pip/_vendor/packaging/markers.py | 304 +++++++++++++++++++++
 1 file changed, 304 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/packaging/py.typed           | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_vendor/packaging/requirements.py          | 146 +++++++++++++++++++++
 1 file changed, 146 insertions(+)
 .../pip/_vendor/packaging/specifiers.py            | 802 +++++++++++++++++++++
 1 file changed, 802 insertions(+)
 .../site-packages/pip/_vendor/packaging/tags.py    | 487 +++++++++++++++++++++
 1 file changed, 487 insertions(+)
 .../site-packages/pip/_vendor/packaging/utils.py   | 136 +++++++++++++++++++++
 1 file changed, 136 insertions(+)
 .../site-packages/pip/_vendor/packaging/version.py | 504 +++++++++++++++++++++
 1 file changed, 504 insertions(+)
 .../pip/_vendor/pkg_resources/__init__.py          | 3361 ++++++++++++++++++++
 1 file changed, 3361 insertions(+)
 .../pip/_vendor/platformdirs/__init__.py           | 566 +++++++++++++++++++++
 1 file changed, 566 insertions(+)
 .../pip/_vendor/platformdirs/__main__.py           | 53 ++++++++++++++++++++++
 1 file changed, 53 insertions(+)
 .../pip/_vendor/platformdirs/android.py            | 210 +++++++++++++++++++++
 1 file changed, 210 insertions(+)
 .../site-packages/pip/_vendor/platformdirs/api.py  | 223 +++++++++++++++++++++
 1 file changed, 223 insertions(+)
 .../pip/_vendor/platformdirs/macos.py              | 91 ++++++++++++++++++++++
 1 file changed, 91 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/platformdirs/py.typed        | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/platformdirs/unix.py | 223 +++++++++++++++++++++
 1 file changed, 223 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/platformdirs/version.py  | 4 ++++
 1 file changed, 4 insertions(+)
 .../pip/_vendor/platformdirs/windows.py            | 255 +++++++++++++++++++++
 1 file changed, 255 insertions(+)
 .../site-packages/pip/_vendor/pygments/__init__.py | 82 ++++++++++++++++++++++
 1 file changed, 82 insertions(+)
 .../site-packages/pip/_vendor/pygments/__main__.py      | 17 +++++++++++++++++
 1 file changed, 17 insertions(+)
 .../site-packages/pip/_vendor/pygments/cmdline.py  | 668 +++++++++++++++++++++
 1 file changed, 668 insertions(+)
 .../site-packages/pip/_vendor/pygments/console.py  | 70 ++++++++++++++++++++++
 1 file changed, 70 insertions(+)
 .../site-packages/pip/_vendor/pygments/filter.py   | 71 ++++++++++++++++++++++
 1 file changed, 71 insertions(+)
 .../pip/_vendor/pygments/filters/__init__.py       | 940 +++++++++++++++++++++
 1 file changed, 940 insertions(+)
 .../pip/_vendor/pygments/formatter.py              | 124 +++++++++++++++++++++
 1 file changed, 124 insertions(+)
 .../pip/_vendor/pygments/formatters/__init__.py    | 158 +++++++++++++++++++++
 1 file changed, 158 insertions(+)
 .../pip/_vendor/pygments/formatters/_mapping.py    | 23 ++++++++++++++++++++++
 1 file changed, 23 insertions(+)
 .../pip/_vendor/pygments/formatters/bbcode.py      | 108 +++++++++++++++++++++
 1 file changed, 108 insertions(+)
 .../pip/_vendor/pygments/formatters/groff.py       | 170 +++++++++++++++++++++
 1 file changed, 170 insertions(+)
 .../pip/_vendor/pygments/formatters/html.py        | 989 +++++++++++++++++++++
 1 file changed, 989 insertions(+)
 .../pip/_vendor/pygments/formatters/img.py         | 645 +++++++++++++++++++++
 1 file changed, 645 insertions(+)
 .../pip/_vendor/pygments/formatters/irc.py         | 154 +++++++++++++++++++++
 1 file changed, 154 insertions(+)
 .../pip/_vendor/pygments/formatters/latex.py       | 521 +++++++++++++++++++++
 1 file changed, 521 insertions(+)
 .../pip/_vendor/pygments/formatters/other.py       | 161 +++++++++++++++++++++
 1 file changed, 161 insertions(+)
 .../pip/_vendor/pygments/formatters/pangomarkup.py | 83 ++++++++++++++++++++++
 1 file changed, 83 insertions(+)
 .../pip/_vendor/pygments/formatters/rtf.py         | 146 +++++++++++++++++++++
 1 file changed, 146 insertions(+)
 .../pip/_vendor/pygments/formatters/svg.py         | 188 +++++++++++++++++++++
 1 file changed, 188 insertions(+)
 .../pip/_vendor/pygments/formatters/terminal.py    | 127 +++++++++++++++++++++
 1 file changed, 127 insertions(+)
 .../pip/_vendor/pygments/formatters/terminal256.py | 338 +++++++++++++++++++++
 1 file changed, 338 insertions(+)
 .../site-packages/pip/_vendor/pygments/lexer.py    | 943 +++++++++++++++++++++
 1 file changed, 943 insertions(+)
 .../pip/_vendor/pygments/lexers/__init__.py        | 362 +++++++++++++++++++++
 1 file changed, 362 insertions(+)
 .../pip/_vendor/pygments/lexers/_mapping.py        | 559 +++++++++++++++++++++
 1 file changed, 559 insertions(+)
 .../pip/_vendor/pygments/lexers/python.py          | 1198 ++++++++++++++++++++
 1 file changed, 1198 insertions(+)
 .../site-packages/pip/_vendor/pygments/modeline.py | 43 ++++++++++++++++++++++
 1 file changed, 43 insertions(+)
 .../site-packages/pip/_vendor/pygments/plugin.py   | 88 ++++++++++++++++++++++
 1 file changed, 88 insertions(+)
 .../site-packages/pip/_vendor/pygments/regexopt.py | 91 ++++++++++++++++++++++
 1 file changed, 91 insertions(+)
 .../site-packages/pip/_vendor/pygments/scanner.py  | 104 +++++++++++++++++++++
 1 file changed, 104 insertions(+)
 .../pip/_vendor/pygments/sphinxext.py              | 217 +++++++++++++++++++++
 1 file changed, 217 insertions(+)
 .../site-packages/pip/_vendor/pygments/style.py    | 197 +++++++++++++++++++++
 1 file changed, 197 insertions(+)
 .../pip/_vendor/pygments/styles/__init__.py        | 103 +++++++++++++++++++++
 1 file changed, 103 insertions(+)
 .../site-packages/pip/_vendor/pygments/token.py    | 213 +++++++++++++++++++++
 1 file changed, 213 insertions(+)
 .../pip/_vendor/pygments/unistring.py              | 153 +++++++++++++++++++++
 1 file changed, 153 insertions(+)
 .../site-packages/pip/_vendor/pygments/util.py     | 330 +++++++++++++++++++++
 1 file changed, 330 insertions(+)
 .../pip/_vendor/pyparsing/__init__.py              | 322 +++++++++++++++++++++
 1 file changed, 322 insertions(+)
 .../site-packages/pip/_vendor/pyparsing/actions.py | 217 +++++++++++++++++++++
 1 file changed, 217 insertions(+)
 .../site-packages/pip/_vendor/pyparsing/common.py  | 432 +++++++++++++++++++++
 1 file changed, 432 insertions(+)
 .../site-packages/pip/_vendor/pyparsing/core.py    | 6115 ++++++++++++++++++++
 1 file changed, 6115 insertions(+)
 .../pip/_vendor/pyparsing/diagram/__init__.py      | 656 +++++++++++++++++++++
 1 file changed, 656 insertions(+)
 .../pip/_vendor/pyparsing/exceptions.py            | 299 +++++++++++++++++++++
 1 file changed, 299 insertions(+)
 .../site-packages/pip/_vendor/pyparsing/helpers.py | 1100 ++++++++++++++++++++
 1 file changed, 1100 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/pyparsing/py.typed           | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/pyparsing/results.py | 796 +++++++++++++++++++++
 1 file changed, 796 insertions(+)
 .../site-packages/pip/_vendor/pyparsing/testing.py | 331 +++++++++++++++++++++
 1 file changed, 331 insertions(+)
 .../site-packages/pip/_vendor/pyparsing/unicode.py | 361 +++++++++++++++++++++
 1 file changed, 361 insertions(+)
 .../site-packages/pip/_vendor/pyparsing/util.py    | 284 +++++++++++++++++++++
 1 file changed, 284 insertions(+)
 .../pip/_vendor/pyproject_hooks/__init__.py        | 23 ++++++++++++++++++++++
 1 file changed, 23 insertions(+)
 .../site-packages/pip/_vendor/pyproject_hooks/_compat.py          | 8 ++++++++
 1 file changed, 8 insertions(+)
 .../pip/_vendor/pyproject_hooks/_impl.py           | 330 +++++++++++++++++++++
 1 file changed, 330 insertions(+)
 .../_vendor/pyproject_hooks/_in_process/__init__.py    | 18 ++++++++++++++++++
 1 file changed, 18 insertions(+)
 .../pyproject_hooks/_in_process/_in_process.py     | 353 +++++++++++++++++++++
 1 file changed, 353 insertions(+)
 .../site-packages/pip/_vendor/requests/__init__.py | 182 +++++++++++++++++++++
 1 file changed, 182 insertions(+)
 .../site-packages/pip/_vendor/requests/__version__.py      | 14 ++++++++++++++
 1 file changed, 14 insertions(+)
 .../pip/_vendor/requests/_internal_utils.py        | 50 ++++++++++++++++++++++
 1 file changed, 50 insertions(+)
 .../site-packages/pip/_vendor/requests/adapters.py | 538 +++++++++++++++++++++
 1 file changed, 538 insertions(+)
 .../site-packages/pip/_vendor/requests/api.py      | 157 +++++++++++++++++++++
 1 file changed, 157 insertions(+)
 .../site-packages/pip/_vendor/requests/auth.py     | 315 +++++++++++++++++++++
 1 file changed, 315 insertions(+)
 .../site-packages/pip/_vendor/requests/certs.py    | 24 ++++++++++++++++++++++
 1 file changed, 24 insertions(+)
 .../site-packages/pip/_vendor/requests/compat.py   | 67 ++++++++++++++++++++++
 1 file changed, 67 insertions(+)
 .../site-packages/pip/_vendor/requests/cookies.py  | 561 +++++++++++++++++++++
 1 file changed, 561 insertions(+)
 .../pip/_vendor/requests/exceptions.py             | 141 +++++++++++++++++++++
 1 file changed, 141 insertions(+)
 .../site-packages/pip/_vendor/requests/help.py     | 131 +++++++++++++++++++++
 1 file changed, 131 insertions(+)
 .../site-packages/pip/_vendor/requests/hooks.py    | 33 ++++++++++++++++++++++
 1 file changed, 33 insertions(+)
 .../site-packages/pip/_vendor/requests/models.py   | 1034 ++++++++++++++++++++
 1 file changed, 1034 insertions(+)
 .../site-packages/pip/_vendor/requests/packages.py       | 16 ++++++++++++++++
 1 file changed, 16 insertions(+)
 .../site-packages/pip/_vendor/requests/sessions.py | 833 +++++++++++++++++++++
 1 file changed, 833 insertions(+)
 .../pip/_vendor/requests/status_codes.py           | 128 +++++++++++++++++++++
 1 file changed, 128 insertions(+)
 .../pip/_vendor/requests/structures.py             | 99 ++++++++++++++++++++++
 1 file changed, 99 insertions(+)
 .../site-packages/pip/_vendor/requests/utils.py    | 1094 ++++++++++++++++++++
 1 file changed, 1094 insertions(+)
 .../pip/_vendor/resolvelib/__init__.py             | 26 ++++++++++++++++++++++
 1 file changed, 26 insertions(+)
 .../python3.12/site-packages/pip/_vendor/resolvelib/compat/__init__.py    | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/resolvelib/compat/collections_abc.py  | 6 ++++++
 1 file changed, 6 insertions(+)
 .../pip/_vendor/resolvelib/providers.py            | 133 +++++++++++++++++++++
 1 file changed, 133 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/resolvelib/py.typed          | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../pip/_vendor/resolvelib/reporters.py            | 43 ++++++++++++++++++++++
 1 file changed, 43 insertions(+)
 .../pip/_vendor/resolvelib/resolvers.py            | 547 +++++++++++++++++++++
 1 file changed, 547 insertions(+)
 .../pip/_vendor/resolvelib/structs.py              | 170 +++++++++++++++++++++
 1 file changed, 170 insertions(+)
 .../site-packages/pip/_vendor/rich/__init__.py     | 177 +++++++++++++++++++++
 1 file changed, 177 insertions(+)
 .../site-packages/pip/_vendor/rich/__main__.py     | 274 +++++++++++++++++++++
 1 file changed, 274 insertions(+)
 .../site-packages/pip/_vendor/rich/_cell_widths.py | 451 +++++++++++++++++++++
 1 file changed, 451 insertions(+)
 .../site-packages/pip/_vendor/rich/_emoji_codes.py | 3610 ++++++++++++++++++++
 1 file changed, 3610 insertions(+)
 .../pip/_vendor/rich/_emoji_replace.py             | 32 ++++++++++++++++++++++
 1 file changed, 32 insertions(+)
 .../pip/_vendor/rich/_export_format.py             | 76 ++++++++++++++++++++++
 1 file changed, 76 insertions(+)
 .../python3.12/site-packages/pip/_vendor/rich/_extension.py    | 10 ++++++++++
 1 file changed, 10 insertions(+)
 .../site-packages/pip/_vendor/rich/_fileno.py      | 24 ++++++++++++++++++++++
 1 file changed, 24 insertions(+)
 .../site-packages/pip/_vendor/rich/_inspect.py     | 270 +++++++++++++++++++++
 1 file changed, 270 insertions(+)
 .../site-packages/pip/_vendor/rich/_log_render.py  | 94 ++++++++++++++++++++++
 1 file changed, 94 insertions(+)
 .../site-packages/pip/_vendor/rich/_loop.py        | 43 ++++++++++++++++++++++
 1 file changed, 43 insertions(+)
 .../site-packages/pip/_vendor/rich/_null_file.py   | 69 ++++++++++++++++++++++
 1 file changed, 69 insertions(+)
 .../site-packages/pip/_vendor/rich/_palettes.py    | 309 +++++++++++++++++++++
 1 file changed, 309 insertions(+)
 .../python3.12/site-packages/pip/_vendor/rich/_pick.py  | 17 +++++++++++++++++
 1 file changed, 17 insertions(+)
 .../site-packages/pip/_vendor/rich/_ratio.py       | 160 +++++++++++++++++++++
 1 file changed, 160 insertions(+)
 .../site-packages/pip/_vendor/rich/_spinners.py    | 482 +++++++++++++++++++++
 1 file changed, 482 insertions(+)
 .../python3.12/site-packages/pip/_vendor/rich/_stack.py  | 16 ++++++++++++++++
 1 file changed, 16 insertions(+)
 .../site-packages/pip/_vendor/rich/_timer.py          | 19 +++++++++++++++++++
 1 file changed, 19 insertions(+)
 .../pip/_vendor/rich/_win32_console.py             | 662 +++++++++++++++++++++
 1 file changed, 662 insertions(+)
 .../site-packages/pip/_vendor/rich/_windows.py     | 72 ++++++++++++++++++++++
 1 file changed, 72 insertions(+)
 .../pip/_vendor/rich/_windows_renderer.py          | 56 ++++++++++++++++++++++
 1 file changed, 56 insertions(+)
 .../site-packages/pip/_vendor/rich/_wrap.py        | 56 ++++++++++++++++++++++
 1 file changed, 56 insertions(+)
 .../site-packages/pip/_vendor/rich/abc.py          | 33 ++++++++++++++++++++++
 1 file changed, 33 insertions(+)
 .../site-packages/pip/_vendor/rich/align.py        | 311 +++++++++++++++++++++
 1 file changed, 311 insertions(+)
 .../site-packages/pip/_vendor/rich/ansi.py         | 240 +++++++++++++++++++++
 1 file changed, 240 insertions(+)
 .../site-packages/pip/_vendor/rich/bar.py          | 94 ++++++++++++++++++++++
 1 file changed, 94 insertions(+)
 .../site-packages/pip/_vendor/rich/box.py          | 517 +++++++++++++++++++++
 1 file changed, 517 insertions(+)
 .../site-packages/pip/_vendor/rich/cells.py        | 154 +++++++++++++++++++++
 1 file changed, 154 insertions(+)
 .../site-packages/pip/_vendor/rich/color.py        | 622 +++++++++++++++++++++
 1 file changed, 622 insertions(+)
 .../pip/_vendor/rich/color_triplet.py              | 38 ++++++++++++++++++++++
 1 file changed, 38 insertions(+)
 .../site-packages/pip/_vendor/rich/columns.py      | 187 +++++++++++++++++++++
 1 file changed, 187 insertions(+)
 .../site-packages/pip/_vendor/rich/console.py      | 2633 ++++++++++++++++++++
 1 file changed, 2633 insertions(+)
 .../site-packages/pip/_vendor/rich/constrain.py    | 37 ++++++++++++++++++++++
 1 file changed, 37 insertions(+)
 .../site-packages/pip/_vendor/rich/containers.py   | 167 +++++++++++++++++++++
 1 file changed, 167 insertions(+)
 .../site-packages/pip/_vendor/rich/control.py      | 225 +++++++++++++++++++++
 1 file changed, 225 insertions(+)
 .../pip/_vendor/rich/default_styles.py             | 190 +++++++++++++++++++++
 1 file changed, 190 insertions(+)
 .../site-packages/pip/_vendor/rich/diagnose.py     | 37 ++++++++++++++++++++++
 1 file changed, 37 insertions(+)
 .../site-packages/pip/_vendor/rich/emoji.py        | 96 ++++++++++++++++++++++
 1 file changed, 96 insertions(+)
 .../site-packages/pip/_vendor/rich/errors.py       | 34 ++++++++++++++++++++++
 1 file changed, 34 insertions(+)
 .../site-packages/pip/_vendor/rich/file_proxy.py   | 57 ++++++++++++++++++++++
 1 file changed, 57 insertions(+)
 .../site-packages/pip/_vendor/rich/filesize.py     | 89 ++++++++++++++++++++++
 1 file changed, 89 insertions(+)
 .../site-packages/pip/_vendor/rich/highlighter.py  | 232 +++++++++++++++++++++
 1 file changed, 232 insertions(+)
 .../site-packages/pip/_vendor/rich/json.py         | 140 +++++++++++++++++++++
 1 file changed, 140 insertions(+)
 .../site-packages/pip/_vendor/rich/jupyter.py      | 101 +++++++++++++++++++++
 1 file changed, 101 insertions(+)
 .../site-packages/pip/_vendor/rich/layout.py       | 443 +++++++++++++++++++++
 1 file changed, 443 insertions(+)
 .../site-packages/pip/_vendor/rich/live.py         | 375 +++++++++++++++++++++
 1 file changed, 375 insertions(+)
 .../site-packages/pip/_vendor/rich/live_render.py  | 113 +++++++++++++++++++++
 1 file changed, 113 insertions(+)
 .../site-packages/pip/_vendor/rich/logging.py      | 289 +++++++++++++++++++++
 1 file changed, 289 insertions(+)
 .../site-packages/pip/_vendor/rich/markup.py       | 246 +++++++++++++++++++++
 1 file changed, 246 insertions(+)
 .../site-packages/pip/_vendor/rich/measure.py      | 151 +++++++++++++++++++++
 1 file changed, 151 insertions(+)
 .../site-packages/pip/_vendor/rich/padding.py      | 141 +++++++++++++++++++++
 1 file changed, 141 insertions(+)
 .../site-packages/pip/_vendor/rich/pager.py        | 34 ++++++++++++++++++++++
 1 file changed, 34 insertions(+)
 .../site-packages/pip/_vendor/rich/palette.py      | 100 +++++++++++++++++++++
 1 file changed, 100 insertions(+)
 .../site-packages/pip/_vendor/rich/panel.py        | 308 +++++++++++++++++++++
 1 file changed, 308 insertions(+)
 .../site-packages/pip/_vendor/rich/pretty.py       | 994 +++++++++++++++++++++
 1 file changed, 994 insertions(+)
 .../site-packages/pip/_vendor/rich/progress.py     | 1702 ++++++++++++++++++++
 1 file changed, 1702 insertions(+)
 .../site-packages/pip/_vendor/rich/progress_bar.py | 224 +++++++++++++++++++++
 1 file changed, 224 insertions(+)
 .../site-packages/pip/_vendor/rich/prompt.py       | 376 +++++++++++++++++++++
 1 file changed, 376 insertions(+)
 .../site-packages/pip/_vendor/rich/protocol.py     | 42 ++++++++++++++++++++++
 1 file changed, 42 insertions(+)
 /dev/null => .venv/lib/python3.12/site-packages/pip/_vendor/rich/py.typed | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../lib/python3.12/site-packages/pip/_vendor/rich/region.py    | 10 ++++++++++
 1 file changed, 10 insertions(+)
 .../site-packages/pip/_vendor/rich/repr.py         | 149 +++++++++++++++++++++
 1 file changed, 149 insertions(+)
 .../site-packages/pip/_vendor/rich/rule.py         | 130 +++++++++++++++++++++
 1 file changed, 130 insertions(+)
 .../site-packages/pip/_vendor/rich/scope.py        | 86 ++++++++++++++++++++++
 1 file changed, 86 insertions(+)
 .../site-packages/pip/_vendor/rich/screen.py       | 54 ++++++++++++++++++++++
 1 file changed, 54 insertions(+)
 .../site-packages/pip/_vendor/rich/segment.py      | 739 +++++++++++++++++++++
 1 file changed, 739 insertions(+)
 .../site-packages/pip/_vendor/rich/spinner.py      | 137 +++++++++++++++++++++
 1 file changed, 137 insertions(+)
 .../site-packages/pip/_vendor/rich/status.py       | 132 +++++++++++++++++++++
 1 file changed, 132 insertions(+)
 .../site-packages/pip/_vendor/rich/style.py        | 796 +++++++++++++++++++++
 1 file changed, 796 insertions(+)
 .../site-packages/pip/_vendor/rich/styled.py       | 42 ++++++++++++++++++++++
 1 file changed, 42 insertions(+)
 .../site-packages/pip/_vendor/rich/syntax.py       | 948 +++++++++++++++++++++
 1 file changed, 948 insertions(+)
 .../site-packages/pip/_vendor/rich/table.py        | 1002 ++++++++++++++++++++
 1 file changed, 1002 insertions(+)
 .../pip/_vendor/rich/terminal_theme.py             | 153 +++++++++++++++++++++
 1 file changed, 153 insertions(+)
 .../site-packages/pip/_vendor/rich/text.py         | 1307 ++++++++++++++++++++
 1 file changed, 1307 insertions(+)
 .../site-packages/pip/_vendor/rich/theme.py        | 115 +++++++++++++++++++++
 1 file changed, 115 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/rich/themes.py          | 5 +++++
 1 file changed, 5 insertions(+)
 .../site-packages/pip/_vendor/rich/traceback.py    | 756 +++++++++++++++++++++
 1 file changed, 756 insertions(+)
 .../site-packages/pip/_vendor/rich/tree.py         | 251 +++++++++++++++++++++
 1 file changed, 251 insertions(+)
 .../python3.12/site-packages/pip/_vendor/six.py    | 998 +++++++++++++++++++++
 1 file changed, 998 insertions(+)
 .../site-packages/pip/_vendor/tenacity/__init__.py | 608 +++++++++++++++++++++
 1 file changed, 608 insertions(+)
 .../site-packages/pip/_vendor/tenacity/_asyncio.py | 94 ++++++++++++++++++++++
 1 file changed, 94 insertions(+)
 .../site-packages/pip/_vendor/tenacity/_utils.py   | 76 ++++++++++++++++++++++
 1 file changed, 76 insertions(+)
 .../site-packages/pip/_vendor/tenacity/after.py    | 51 ++++++++++++++++++++++
 1 file changed, 51 insertions(+)
 .../site-packages/pip/_vendor/tenacity/before.py   | 46 ++++++++++++++++++++++
 1 file changed, 46 insertions(+)
 .../pip/_vendor/tenacity/before_sleep.py           | 71 ++++++++++++++++++++++
 1 file changed, 71 insertions(+)
 .../site-packages/pip/_vendor/tenacity/nap.py      | 43 ++++++++++++++++++++++
 1 file changed, 43 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/tenacity/py.typed            | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/tenacity/retry.py    | 272 +++++++++++++++++++++
 1 file changed, 272 insertions(+)
 .../site-packages/pip/_vendor/tenacity/stop.py     | 103 +++++++++++++++++++++
 1 file changed, 103 insertions(+)
 .../pip/_vendor/tenacity/tornadoweb.py             | 59 ++++++++++++++++++++++
 1 file changed, 59 insertions(+)
 .../site-packages/pip/_vendor/tenacity/wait.py     | 228 +++++++++++++++++++++
 1 file changed, 228 insertions(+)
 .../python3.12/site-packages/pip/_vendor/tomli/__init__.py    | 11 +++++++++++
 1 file changed, 11 insertions(+)
 .../site-packages/pip/_vendor/tomli/_parser.py     | 691 +++++++++++++++++++++
 1 file changed, 691 insertions(+)
 .../site-packages/pip/_vendor/tomli/_re.py         | 107 +++++++++++++++++++++
 1 file changed, 107 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/tomli/_types.py   | 10 ++++++++++
 1 file changed, 10 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/tomli/py.typed              | 1 +
 1 file changed, 1 insertion(+)
 .../site-packages/pip/_vendor/truststore/__init__.py        | 13 +++++++++++++
 1 file changed, 13 insertions(+)
 .../site-packages/pip/_vendor/truststore/_api.py   | 302 +++++++++++++++++++++
 1 file changed, 302 insertions(+)
 .../site-packages/pip/_vendor/truststore/_macos.py | 501 +++++++++++++++++++++
 1 file changed, 501 insertions(+)
 .../pip/_vendor/truststore/_openssl.py             | 66 ++++++++++++++++++++++
 1 file changed, 66 insertions(+)
 .../pip/_vendor/truststore/_ssl_constants.py       | 31 ++++++++++++++++++++++
 1 file changed, 31 insertions(+)
 .../pip/_vendor/truststore/_windows.py             | 554 +++++++++++++++++++++
 1 file changed, 554 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/truststore/py.typed          | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/typing_extensions.py | 3072 ++++++++++++++++++++
 1 file changed, 3072 insertions(+)
 .../site-packages/pip/_vendor/urllib3/__init__.py  | 102 +++++++++++++++++++++
 1 file changed, 102 insertions(+)
 .../pip/_vendor/urllib3/_collections.py            | 337 +++++++++++++++++++++
 1 file changed, 337 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/urllib3/_version.py        | 2 ++
 1 file changed, 2 insertions(+)
 .../pip/_vendor/urllib3/connection.py              | 572 +++++++++++++++++++++
 1 file changed, 572 insertions(+)
 .../pip/_vendor/urllib3/connectionpool.py          | 1132 ++++++++++++++++++++
 1 file changed, 1132 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/urllib3/contrib/__init__.py  | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../_vendor/urllib3/contrib/_appengine_environ.py  | 36 ++++++++++++++++++++++
 1 file changed, 36 insertions(+)
 .../pip/_vendor/urllib3/contrib/_securetransport/__init__.py              | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../urllib3/contrib/_securetransport/bindings.py   | 519 +++++++++++++++++++++
 1 file changed, 519 insertions(+)
 .../urllib3/contrib/_securetransport/low_level.py  | 397 +++++++++++++++++++++
 1 file changed, 397 insertions(+)
 .../pip/_vendor/urllib3/contrib/appengine.py       | 314 +++++++++++++++++++++
 1 file changed, 314 insertions(+)
 .../pip/_vendor/urllib3/contrib/ntlmpool.py        | 130 +++++++++++++++++++++
 1 file changed, 130 insertions(+)
 .../pip/_vendor/urllib3/contrib/pyopenssl.py       | 518 +++++++++++++++++++++
 1 file changed, 518 insertions(+)
 .../pip/_vendor/urllib3/contrib/securetransport.py | 921 +++++++++++++++++++++
 1 file changed, 921 insertions(+)
 .../pip/_vendor/urllib3/contrib/socks.py           | 216 +++++++++++++++++++++
 1 file changed, 216 insertions(+)
 .../pip/_vendor/urllib3/exceptions.py              | 323 +++++++++++++++++++++
 1 file changed, 323 insertions(+)
 .../site-packages/pip/_vendor/urllib3/fields.py    | 274 +++++++++++++++++++++
 1 file changed, 274 insertions(+)
 .../site-packages/pip/_vendor/urllib3/filepost.py  | 98 ++++++++++++++++++++++
 1 file changed, 98 insertions(+)
 .../lib/python3.12/site-packages/pip/_vendor/urllib3/packages/__init__.py | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../site-packages/pip/_vendor/urllib3/packages/backports/__init__.py      | 0
 1 file changed, 0 insertions(+), 0 deletions(-)
 .../_vendor/urllib3/packages/backports/makefile.py | 51 ++++++++++++++++++++++
 1 file changed, 51 insertions(+)
 .../urllib3/packages/backports/weakref_finalize.py | 155 +++++++++++++++++++++
 1 file changed, 155 insertions(+)
 .../pip/_vendor/urllib3/packages/six.py            | 1076 ++++++++++++++++++++
 1 file changed, 1076 insertions(+)
 .../pip/_vendor/urllib3/poolmanager.py             | 537 +++++++++++++++++++++
 1 file changed, 537 insertions(+)
 .../site-packages/pip/_vendor/urllib3/request.py   | 191 +++++++++++++++++++++
 1 file changed, 191 insertions(+)
 .../site-packages/pip/_vendor/urllib3/response.py  | 879 +++++++++++++++++++++
 1 file changed, 879 insertions(+)
 .../pip/_vendor/urllib3/util/__init__.py           | 49 ++++++++++++++++++++++
 1 file changed, 49 insertions(+)
 .../pip/_vendor/urllib3/util/connection.py         | 149 +++++++++++++++++++++
 1 file changed, 149 insertions(+)
 .../pip/_vendor/urllib3/util/proxy.py              | 57 ++++++++++++++++++++++
 1 file changed, 57 insertions(+)
 .../pip/_vendor/urllib3/util/queue.py              | 22 ++++++++++++++++++++++
 1 file changed, 22 insertions(+)
 .../pip/_vendor/urllib3/util/request.py            | 137 +++++++++++++++++++++
 1 file changed, 137 insertions(+)
 .../pip/_vendor/urllib3/util/response.py           | 107 +++++++++++++++++++++
 1 file changed, 107 insertions(+)
 .../pip/_vendor/urllib3/util/retry.py              | 620 +++++++++++++++++++++
 1 file changed, 620 insertions(+)
 .../site-packages/pip/_vendor/urllib3/util/ssl_.py | 495 +++++++++++++++++++++
 1 file changed, 495 insertions(+)
 .../pip/_vendor/urllib3/util/ssl_match_hostname.py | 159 +++++++++++++++++++++
 1 file changed, 159 insertions(+)
 .../pip/_vendor/urllib3/util/ssltransport.py       | 221 +++++++++++++++++++++
 1 file changed, 221 insertions(+)
 .../pip/_vendor/urllib3/util/timeout.py            | 271 +++++++++++++++++++++
 1 file changed, 271 insertions(+)
 .../site-packages/pip/_vendor/urllib3/util/url.py  | 435 +++++++++++++++++++++
 1 file changed, 435 insertions(+)
 .../site-packages/pip/_vendor/urllib3/util/wait.py | 152 +++++++++++++++++++++
 1 file changed, 152 insertions(+)
 .../site-packages/pip/_vendor/vendor.txt           | 24 ++++++++++++++++++++++
 1 file changed, 24 insertions(+)
 .../pip/_vendor/webencodings/__init__.py           | 342 +++++++++++++++++++++
 1 file changed, 342 insertions(+)
 .../pip/_vendor/webencodings/labels.py             | 231 +++++++++++++++++++++
 1 file changed, 231 insertions(+)
 .../pip/_vendor/webencodings/mklabels.py           | 59 ++++++++++++++++++++++
 1 file changed, 59 insertions(+)
 .../pip/_vendor/webencodings/tests.py              | 153 +++++++++++++++++++++
 1 file changed, 153 insertions(+)
 .../pip/_vendor/webencodings/x_user_defined.py     | 325 +++++++++++++++++++++
 1 file changed, 325 insertions(+)
 /dev/null => .venv/lib/python3.12/site-packages/pip/py.typed | 4 ++++
 1 file changed, 4 insertions(+)
 /dev/null => .venv/pyvenv.cfg | 5 +++++
 1 file changed, 5 insertions(+)
```

## Required Commands

```text
$ test -x .venv/bin/python
[exit status: 0]

$ . .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'
/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/worktree/.venv/bin/python: No module named core_terminal_demo
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/agent-output.md`
- Events: `docs/benchmarks/live-flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 6
tool_execution_progress: 2
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 1796
diff_chars: 6704541
diff_files_changed: 537
tool_executions: 6
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 111
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 15
closeout_tool_evidence: tool evidence: records=15 completed=6 failed=9 denied=0 validation=2 closeout=2 repair=9 changed=0 workflows=code_change commands=pwd | which python3 && python3 --version | python3 -c "import core_terminal_demo" 2>&1 || echo "IMPORT_FAILED"
runtime_diet: prompt=8250 tool_schema=4300 tools=20 workflow=guarded closeout=full validation=failed:1/2
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
trace_event_types: stop.check,agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: none
behavior_assertion_status: none
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=24 latest=runtime_diet_report decision=23 latest=risk_signal_assessed permission=0 latest=none tool_execution=20 latest=tool_completed state_update=34 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=5 risky_tool_reviewed=5 risky_tool_missing_action_review=none gate_outcomes=total=9, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=6 latest_action_score=12 low_action_score_count=1 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=4 provider_protocol_repairs=42 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=8 context_zones=4 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 5
risky_tool_reviewed: 5
risky_tool_missing_action_review: none
gate_outcomes: total=9, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:failed:protective_block
gate_outcome_total: 9
gate_outcome_protective_blocks: 3
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 6
gate_outcome_failure_owners: none
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 8
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 1/2 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 1
repeated_action_count: 0
failed_action_count: 2
user_question_count: 2
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 6
llm_call_count: 4
warning: max_files_changed_exceeded
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: mixed
outcome_score: 15
process_score: 95
efficiency_score: 74
agent_score: 51
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,invalid_action,failed_actions,user_questions
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=24 latest=runtime_diet_report decision=23 latest=risk_signal_assessed permission=0 latest=none tool_execution=20 latest=tool_completed state_update=34 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=5 risky_tool_reviewed=5 risky_tool_missing_action_review=none gate_outcomes=total=9, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=6 latest_action_score=12 low_action_score_count=1 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=4 provider_protocol_repairs=42 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=8 context_zones=4 completion_contract=failed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 5
risky_tool_reviewed: 5
risky_tool_missing_action_review: none
gate_outcomes: total=9, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:failed:protective_block
gate_outcome_total: 9
gate_outcome_protective_blocks: 3
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 6
gate_outcome_failure_owners: none
agent_loop_steps: 8
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 1/2 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: failure_pattern
memory_proposal_evidence_items: 11
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 2
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P1
latest_top_importance_score: 0.7050000429153442
latest_top_weight_share: 0.2601476013660431
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 15
closeout_tool_evidence: tool evidence: records=15 completed=6 failed=9 denied=0 validation=2 closeout=2 repair=9 changed=0 workflows=code_change commands=pwd | which python3 && python3 --version | python3 -c "import core_terminal_demo" 2>&1 || echo "IMPORT_FAILED"
runtime_diet: prompt=8250 tool_schema=4300 tools=20 workflow=guarded
attention: required commands did not pass in the harness
```

## Human Review

- accepted: TODO
- task_success: TODO
- mainline_hit: TODO
- plan_coverage: TODO
- rework_count: TODO
- tool_efficiency: TODO
- diff_discipline: TODO
- closeout_accuracy: TODO
- notes: TODO

## Run Bundle

- Bundle: `docs/benchmarks/live-flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/run-bundle`
- Task: `docs/benchmarks/live-flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-fix-terminal-preflight5-20260527-130500/core-terminal-install-run/run-bundle/final_report.md`
