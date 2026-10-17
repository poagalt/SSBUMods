#!/usr/bin/env bash
# REA (github.com/morluto/rea) for this project: installed in WSL, used from Claude Code on Windows.
#
#   bash tools/rea-wsl.sh setup     # install or repair everything below; safe to re-run
#   bash tools/rea-wsl.sh update    # newest rea-agents release, then refresh the skill and check
#   bash tools/rea-wsl.sh check     # readiness of both engines, the skill and the registration
#
# WHY WSL. REA's native analysis is Ghidra or Hopper. Its Windows Ghidra mode admits only x86/x64
# PE files, and Hopper has no Windows build, so on Windows it cannot open AArch64 Switch code at
# all. The Linux providers open the ELFs tools/rea-elf.py builds. Claude Code stays on Windows and
# starts the server as `wsl.exe -d Ubuntu --exec /usr/local/bin/rea-mcp`; paths handed to REA are
# therefore Linux paths (/mnt/c/Users/poaga/SSBUMods/work/rea/main.elf). See handbook/rea.md.
#
# What `setup` installs, all inside the Ubuntu distro (which runs as root):
#   OpenJDK 21 (apt), Node 24 LTS (nodejs.org tarball, SHA-256 checked), Ghidra 12.1.4 (NSA
#   release zip, SHA-256 checked), rea-agents from npm, Hopper's Linux demo (through REA's own
#   setup, which checks the package digest), and the launcher /usr/local/bin/rea-mcp.
# On Windows: the REA skill in %USERPROFILE%\.claude\skills\reverse-engineer-anything, copied from
# the installed package so it always describes the server actually running, and a user-scope MCP
# registration named `rea`.
set -uo pipefail
export MSYS_NO_PATHCONV=1
DISTRO=Ubuntu
REA_VERSION="${REA_VERSION:-6.4.0}"
CLAUDE="$USERPROFILE/.local/bin/claude.exe"
WSL=(wsl.exe -d "$DISTRO" --exec)
WIN_SKILL=/mnt/c/Users/poaga/.claude/skills/reverse-engineer-anything

wsl_bash() { "${WSL[@]}" bash -s -- "$@"; }

install_wsl() {
    wsl_bash "$REA_VERSION" <<'EOF'
set -euo pipefail
REA_VERSION="$1"
export DEBIAN_FRONTEND=noninteractive
step() { echo "== $*"; }

step "apt: JDK 21 and tools"
apt-get update -qq
apt-get install -y -qq openjdk-21-jdk-headless unzip xz-utils curl ca-certificates binutils >/dev/null

NODE_VER=v24.21.0
NODE_SHA=fd8e59d5a511510f6a298afb548f18c7d2b1be404d8b4a27d94fbe49f56cb2d6
if [ ! -x /opt/node-$NODE_VER-linux-x64/bin/node ]; then
    step "Node $NODE_VER"
    mkdir -p /opt/downloads/node && cd /opt/downloads/node
    curl -fsSLo node.tar.xz https://nodejs.org/dist/$NODE_VER/node-$NODE_VER-linux-x64.tar.xz
    echo "$NODE_SHA  node.tar.xz" | sha256sum -c -
    tar -xJf node.tar.xz -C /opt
fi
for b in node npm npx corepack; do ln -sfn /opt/node-$NODE_VER-linux-x64/bin/$b /usr/local/bin/$b; done

G=ghidra_12.1.4_PUBLIC_20260921.zip
G_SHA=ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db
if [ ! -x /opt/ghidra_12.1.4_PUBLIC/support/analyzeHeadless ]; then
    step "Ghidra 12.1.4"
    mkdir -p /opt/downloads/ghidra && cd /opt/downloads/ghidra
    curl -fsSLo $G https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_12.1.4_build/$G
    echo "$G_SHA  $G" | sha256sum -c -
    unzip -q $G -d /opt
fi

have="$(/usr/local/bin/rea --version 2>/dev/null || true)"
if [ "$have" != "$REA_VERSION" ]; then
    step "rea-agents $REA_VERSION (had: ${have:-none})"
    npm install --global "rea-agents@$REA_VERSION" --no-fund --no-audit
fi
ln -sfn "$(npm prefix -g)/bin/rea" /usr/local/bin/rea

step "launcher"
# The server's environment lives in one file that the launcher and every check source alike, so
# a `doctor` run sees exactly what the MCP server sees.
cat > /usr/local/bin/rea-mcp-env <<'ENV'
# Sourced by /usr/local/bin/rea-mcp; written by SSBUMods/tools/rea-wsl.sh.
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export HOME=/root
export GHIDRA_INSTALL_DIR=/opt/ghidra_12.1.4_PUBLIC
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
# Both engines open an ELF, and REA refuses to guess between them.
export REA_ANALYSIS_PROVIDER=ghidra
# main.elf needs far more than Ghidra's 2G default heap and REA's 330 s default startup
# deadline; handbook/rea.md has the measured numbers.
export GHIDRA_HEADLESS_MAXMEM=8G
export REA_GHIDRA_STARTUP_TIMEOUT_MS=10800000
export REA_HOPPER_STARTUP_TIMEOUT_MS=1800000
ENV
cat > /usr/local/bin/rea-mcp <<'LAUNCHER'
#!/bin/sh
# REA's MCP server for Claude Code on Windows:  wsl.exe -d Ubuntu --exec /usr/local/bin/rea-mcp
# Written by SSBUMods/tools/rea-wsl.sh; the environment is in /usr/local/bin/rea-mcp-env.
. /usr/local/bin/rea-mcp-env
exec /usr/local/bin/rea mcp
LAUNCHER
chmod 644 /usr/local/bin/rea-mcp-env
chmod 755 /usr/local/bin/rea-mcp

. /usr/local/bin/rea-mcp-env
if ! rea doctor --provider hopper --json >/dev/null 2>&1; then
    step "Hopper demo (REA setup verifies the package digest)"
    rea setup --yes --install-hopper --skill=false --json >/tmp/rea-hopper-setup.json
fi
EOF
}

refresh_skill() {
    wsl_bash "$WIN_SKILL" <<'EOF'
set -euo pipefail
D="$1"
P="$(npm root -g)/rea-agents/skills/reverse-engineer-anything"
[ -f "$P/SKILL.md" ] || { echo "no bundled skill at $P" >&2; exit 1; }
rm -rf "$D" && mkdir -p "$D" && cp -r "$P/." "$D/"
echo "== skill copied from $P"
EOF
}

# The per-server "timeout" is not optional. A cold Ghidra import sends no MCP progress under
# rea-agents 6.4.0, and Claude Code aborts a call that is silent for its idle window (30 min by
# default): "sent no response or progress for 60s; aborting", measured with the window forced
# down. The per-server timeout replaces that window for this server only. main.elf's cold query
# takes 65 min, so it gets the same 3 h REA_GHIDRA_STARTUP_TIMEOUT_MS gives Ghidra.
# Written from Git Bash on purpose: Windows PowerShell 5.1 strips the quotes out of a JSON
# argument to a native program, and add-json then rejects it as "Invalid input".
REA_ENTRY="{\"type\":\"stdio\",\"command\":\"wsl.exe\",\"args\":[\"-d\",\"$DISTRO\",\"--exec\",\"/usr/local/bin/rea-mcp\"],\"timeout\":10800000}"

register() {
    local have
    have="$(python -c 'import json,os,sys; d=json.load(open(os.path.join(os.environ["USERPROFILE"],".claude.json"),encoding="utf-8")); print(json.dumps(d.get("mcpServers",{}).get("rea"),separators=(",",":")))' 2>/dev/null)"
    if [ "$have" = "$REA_ENTRY" ]; then
        echo "== MCP registration 'rea' already present"
        return
    fi
    [ "$have" != "null" ] && [ -n "$have" ] && "$CLAUDE" mcp remove rea -s user
    "$CLAUDE" mcp add-json --scope user rea "$REA_ENTRY"
}

check() {
    wsl_bash <<'EOF'
. /usr/local/bin/rea-mcp-env
echo "rea $(rea --version)"
for p in ghidra hopper; do
    rea doctor --provider $p --json | node -e '
        let s = "";
        process.stdin.on("data", (d) => (s += d)).on("end", () => {
            const d = JSON.parse(s);
            console.log(process.argv[1], d.healthy ? "healthy" : "NOT healthy");
            for (const c of d.scope_checks.filter((c) => !c.ok)) {
                console.log("  " + c.name + ": " + (c.remediation || c.detail));
            }
        });' $p
done
EOF
    echo "windows skill $(grep -m1 -o 'version: "[0-9]*"' "$USERPROFILE/.claude/skills/reverse-engineer-anything/SKILL.md")"
    "$CLAUDE" mcp get rea | grep -E "Status|Command|Args"
}

case "${1:-}" in
    setup)
        install_wsl && refresh_skill && register && check
        ;;
    update)
        wsl_bash <<<'. /usr/local/bin/rea-mcp-env; rea update' && refresh_skill && check
        echo "Restart Claude sessions that should use the new server."
        ;;
    check)
        check
        ;;
    *)
        sed -n '2,7p' "$0"
        exit 2
        ;;
esac
