# G2-J (nach W2 testprofil.sh): ein Prüfskript darf ausschließlich sein
# eigenes Testprofil unter nachweis/gui löschen.
testprofil_neu() {
    local basis name ziel
    basis=$(realpath -e -- "${1:?Testordner}" 2>/dev/null) ||
        { echo "testprofil: Testordner fehlt: $1" >&2; exit 2; }
    name=${2-}
    case $name in
    '' | .* | */* | *[!A-Za-z0-9._-]* | rt | bin)
        echo "testprofil: unzulässiger Profilname: '$name'" >&2; exit 2 ;;
    esac
    [ "$basis" = /home/user/projects/querencia-runtime-wt-lautstaerke/nachweis/gui ] ||
        { echo "testprofil: kein zugelassener Testordner: $basis" >&2; exit 2; }
    ziel=$basis/$name
    if [ -L "$ziel" ]; then echo "testprofil: Link, nichts gelöscht: $ziel" >&2; exit 2; fi
    if [ -e "$ziel" ]; then
        if [ -d "$ziel" ] && [ -f "$ziel/.querencia-testprofil" ]; then
            chmod -R u+w -- "$ziel"; rm -rf -- "$ziel"
        else
            echo "testprofil: kein Testprofil, nichts gelöscht: $ziel" >&2; exit 2
        fi
    fi
    mkdir -p -- "$ziel"; : >"$ziel/.querencia-testprofil"
}
