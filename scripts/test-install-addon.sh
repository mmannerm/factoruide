#!/bin/sh
# Exercise a real upgrade in a disposable fake WoW tree, never the user's installation.
set -eu
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
unset FG_INSTALL_CHARACTER_TALENTS
export FG_WOW_DIR="$tmp" FG_CLIENT=_forever_ FG_INSTALL_DATA=0
addons="$tmp/_forever_/Interface/AddOns"
sh scripts/install-addon.sh >/dev/null
test ! -e "$addons/Factoruide_CharacterTalents"
FG_INSTALL_CHARACTER_TALENTS=1 sh scripts/install-addon.sh >/dev/null
test -f "$addons/Factoruide_CharacterTalents/Presets.lua"
printf '%s\n' 'local personal build, preserve exactly' > "$addons/Factoruide_CharacterTalents/Builds.lua"
cp "$addons/Factoruide_CharacterTalents/Builds.lua" "$tmp/builds"
mkdir -p "$addons/Factoruide_Data"
printf '%s\n' 'existing generated guides' > "$addons/Factoruide_Data/Data.lua"
cp "$addons/Factoruide_Data/Data.lua" "$tmp/data"
printf '%s\n' 'existing settings' > "$addons/Factoruide_Guide/Config.lua"
cp "$addons/Factoruide_Guide/Config.lua" "$tmp/config"
sv="$tmp/_forever_/WTF/Account/TEST/SavedVariables"
mkdir -p "$sv"
printf '%s\n' 'existing profile and progress' > "$sv/Factoruide_Profile.lua"
cp "$sv/Factoruide_Profile.lua" "$tmp/saved"
printf '%s\n' 'old runtime' > "$addons/Factoruide_CharacterTalents/Runtime.lua"
sh scripts/install-addon.sh >/dev/null
cmp "$tmp/builds" "$addons/Factoruide_CharacterTalents/Builds.lua"
cmp "$tmp/data" "$addons/Factoruide_Data/Data.lua"
cmp "$tmp/config" "$addons/Factoruide_Guide/Config.lua"
cmp "$tmp/saved" "$sv/Factoruide_Profile.lua"
cmp addon/Factoruide_CharacterTalents/Runtime.lua "$addons/Factoruide_CharacterTalents/Runtime.lua"
cmp addon/Factoruide_CharacterTalents/Presets.lua "$addons/Factoruide_CharacterTalents/Presets.lua"
cmp addon/Factoruide_CharacterTalents/Factoruide_CharacterTalents.toc "$addons/Factoruide_CharacterTalents/Factoruide_CharacterTalents.toc"
echo "addon installation ok: optional first install, existing plans/guides/settings preserved on upgrade"
