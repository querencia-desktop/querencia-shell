# Schutz für die isolierten Prüfskripte (wird mit "." eingebunden): Ein
# Prüfskript darf ausschließlich sein eigenes Testprofil löschen.
#
#   testprofil_neu <testordner> <profilname>
#
# - <profilname> ist ein einfacher Name (Buchstaben, Ziffern, . _ -; kein
#   Schrägstrich, beginnt nicht mit einem Punkt); das Profil ist
#   <testordner>/<profilname>.
# - <testordner> muss (nach realpath) ein zugelassener Testordner sein.
# - Ein vorhandenes Profil wird nur gelöscht, wenn es ein echtes Verzeichnis
#   ist, das ein Test angelegt hat (Marke .querencia-testprofil oder logs/
#   von isoliert.py), nie ein symbolischer Link.
# Danach existiert das leere Profil mit der Marke. Jeder Verstoß: exit 2,
# nichts gelöscht.
testprofil_neu() {
    local basis name ziel
    basis=$(realpath -e -- "${1:?Testordner}" 2>/dev/null) ||
        { echo "testprofil: Testordner fehlt: $1" >&2; exit 2; }
    name=${2-}
    case $name in
    '' | .* | */* | *[!A-Za-z0-9._-]*)
        echo "testprofil: unzulässiger Profilname: '$name'" >&2; exit 2 ;;
    esac
    case $basis in
    ~/projects/querencia-panel-werkstatt/gui-test-nacht | \
    ~/projects/querencia-panel-werkstatt/hmp-[0-9][0-9]/gui-test* | \
    ~/projects/querencia-panel-werkstatt/arbeitsflaechen-4/nachweis | \
    ~/.werkstatt/cache/scratch/?* | \
    ~/projects/querencia-werkstatt-w1-desktop/nachweis/gui-test* | \
    ~/projects/querencia-werkstatt-w1-desktop/hmp10a/nachweis/gui-test* | \
    ~/projects/querencia-werkstatt-w1-desktop/hmp10b/nachweis/gui-test* | \
    ~/projects/querencia-werkstatt-w1-desktop/qe01/nachweis/gui-test*) ;;
    ~/projects/querencia-panel-werkstatt/sitzung)
        case $name in
        test-*) ;;
        *) echo "testprofil: in sitzung/ nur test-*: '$name'" >&2; exit 2 ;;
        esac ;;
    *) echo "testprofil: kein zugelassener Testordner: $basis" >&2; exit 2 ;;
    esac
    ziel=$basis/$name
    if [ -L "$ziel" ]; then
        echo "testprofil: Profil ist ein Link, nichts gelöscht: $ziel" >&2; exit 2
    fi
    if [ -e "$ziel" ]; then
        if [ -d "$ziel" ] && { [ -f "$ziel/.querencia-testprofil" ] || [ -d "$ziel/logs" ]; }; then
            # schreibgeschützte Ablagen im Profil (installieren.sh) erst freigeben
            chmod -R u+w -- "$ziel"
            rm -rf -- "$ziel" || { echo "testprofil: konnte $ziel nicht löschen" >&2; exit 2; }
        else
            echo "testprofil: kein Testprofil, nichts gelöscht: $ziel" >&2; exit 2
        fi
    fi
    mkdir -p -- "$ziel"
    : >"$ziel/.querencia-testprofil"
}
