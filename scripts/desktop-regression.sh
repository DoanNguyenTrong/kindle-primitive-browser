#!/bin/sh
# Desktop regression suite for the browser engine and UI.
#
# Starts the fixture server (tests/fixtures/server.py) on a free local port, drives the
# desktop build (kb-desktop) through scripted commands, and checks the log for the
# expected results. Exit status is 0 only if every scenario passes.
#
#   scripts/desktop-regression.sh [--network]
#
# --network also runs scenarios against live sites (Hacker News, Wikipedia). Those depend
# on the internet and on the sites' markup, so they are opt-in.
#
# Environment: KB_BIN (default target/release/kb-desktop), KB_PORT (default 8780),
# KB_TEST_OUT (default: a temporary folder; frames and logs are kept there).
set -u

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN=${KB_BIN:-$ROOT/target/release/kb-desktop}
PORT=${KB_PORT:-8780}
FIXTURES=$ROOT/tests/fixtures
OUT=${KB_TEST_OUT:-$(mktemp -d)}
FONTS=$(ls "$ROOT"/assets/fonts/*.ttf | paste -sd, -)
BASE=http://127.0.0.1:$PORT
NETWORK=0
[ "${1:-}" = "--network" ] && NETWORK=1
mkdir -p "$OUT" || exit 2

[ -x "$BIN" ] || { echo "missing binary: $BIN (build with: cargo build --release -p kb-desktop)"; exit 2; }

python3 "$FIXTURES/server.py" "$PORT" >"$OUT/server.log" 2>&1 &
SERVER=$!
trap 'kill $SERVER 2>/dev/null' EXIT INT TERM
sleep 1

pass=0
fail=0
failed=""

# run NAME SCRIPT EXPECT...  - runs the script, then checks each EXPECT substring is in the log.
run() {
    name=$1
    script=$2
    shift 2
    data=$(mktemp -d)
    printf '%s\n' "$script" | KB_FONTS="$FONTS" KB_DATA="$data" KB_OUT="$OUT/$name.png" \
        timeout 300 "$BIN" >"$OUT/$name.log" 2>&1
    rc=$?
    ok=1
    if [ $rc -ne 0 ]; then
        ok=0
        echo "  exit status $rc"
    fi
    for want in "$@"; do
        if ! grep -qF -- "$want" "$OUT/$name.log"; then
            ok=0
            echo "  missing in log: $want"
        fi
    done
    if [ $ok -eq 1 ]; then
        pass=$((pass + 1))
        echo "PASS $name"
    else
        fail=$((fail + 1))
        failed="$failed $name"
        echo "FAIL $name  (log: $OUT/$name.log)"
    fi
}

# --- browser pages and navigation -------------------------------------------------
run home "open kbp://home
quit" "Home | kbp://home"

run js_button_keeps_page "open $BASE/index.html
tapsel #b1
quit" "[tapsel] action" "timer ran | $BASE/index.html"

run link_navigates "open $BASE/index.html
tapsel a[href\$=\"other.html\"]
quit" "Other page | $BASE/other.html"

run back_and_forward "open $BASE/index.html
open $BASE/plain.html
back
fwd
quit" "[back]" "[forward] action" "plain.html"

run reload "open $BASE/plain.html
reload
quit" "[reload] action"

run not_found_shows_error "open $BASE/missing.html
quit" "Could not load this page"

run long_press_opens_new_tab "open $BASE/index.html
longsel a[href\$=\"other.html\"]
quit" "Other page | $BASE/other.html"

# --- forms and input --------------------------------------------------------------
run form_get_submit "open $BASE/form.html
tapsel input[name=q]
tapkey h
tapkey i
tapsel a[href=\"kb:go\"]
quit" "$BASE/search?q=hi"

run form_post_submit "open $BASE/post.html
tapsel input[name=city]
tapkey h
tapkey i
tapsel a[href=\"kb:go\"]
text p
quit" "received | $BASE/submit" "POST city=hi"

run keyboard_caret_insert "open $BASE/form.html
tapsel input[name=q]
tapkey a
tapkey b
tapkey c
tapsel a[href=\"kb:left\"]
tapkey x
tapsel a[href=\"kb:go\"]
quit" "$BASE/search?q=abxc"

# --- DOM and JavaScript APIs ------------------------------------------------------
run dom_apis "open $BASE/dom.html
text #res
quit" "byClass:2" "cl:true" "data:main" "closest:box"

run js_platform_apis "open $BASE/apis.html
text #out
quit" "load fired" "abort:true" "uuid:36" "url:example.org/a/b:two" "b64:aGk=/hi"

# --- reading, saving, bookmarks, tabs ---------------------------------------------
run reading_view "open $BASE/article.html
action reader
count h1
quit" "[count] h1: 1"

run save_offline_and_reopen "open $BASE/article.html
action save
action saved
tapsel a[href^=\"kbp://saved/\"]
assert title Test article
text h1
quit" "[text] h1: Test article"

run bookmark_add_and_list "open $BASE/plain.html
action mark
action marks
quit" "Bookmarks | kbp://bookmarks"

run new_tab_and_switch "open kbp://home
action newtab
tabstep -1
quit" "Home | kbp://home"

run tabs_sheet_opens "open kbp://home
action tabs
quit" "[action] action"

# Home has one search box (the address bar); its sections fold away.
run home_single_search_box "open kbp://home
count .search
quit" "[count] .search: 0"

run home_section_toggle "open kbp://home
action homeshow:bm
count .tile
quit" "[count] .tile: 0" "[ui] action kb:homeshow:bm"

run bookmark_reorder_and_remove "open $BASE/plain.html
action mark
action marks
action bmdown:0
action rm:0
quit" "[ui] action kb:bmdown:0" "[ui] action kb:rm:0"

# --- menu and settings ------------------------------------------------------------
run menu_scrolls "open kbp://home
action menu
scroll 500
quit" "[scroll] action"

run text_size_and_night "open $BASE/plain.html
action display
action font:+
action width:800
action nighton
action nightoff
quit" "[ui] action kb:display" "[ui] action kb:font:+" "[ui] action kb:width:800" "[ui] action kb:nightoff"

# Menu taps go through the sheet hit test in device pixels on the 1264x1680 screen.
run menu_toggle_keeps_sheet "open kbp://home
action menu
tap 300 1460
tap 300 1460
quit" "[ui] sheet=Menu tap (300,1460) -> Some(\"kb:night\")" "[ui] sheet=Menu tap (300,1460) -> Some(\"kb:night\")"

# Images: a switch in the menu. Off skips image requests (no network, no decode); on fetches them.
run images_on_by_default "open $BASE/images.html
count img
quit" "[count] img: 1"

run images_off_skips_fetch "open $BASE/images.html
action images
count img
quit" "[ui] action kb:images" "[net] image skipped (images off)"

run menu_images_row_keeps_sheet "open $BASE/images.html
action menu
tap 300 1368
quit" "[ui] sheet=Menu tap (300,1368) -> Some(\"kb:images\")" "[ui] action kb:images (sheet=Some(Menu)"

run menu_navigation_closes_sheet "open kbp://home
action menu
tap 300 1100
quit" "[ui] action kb:reload (sheet=None"

run menu_exit_button "open kbp://home
action menu
tapsel a[href=\"kb:exit\"]" "exit requested"

run address_bar_hint_on_home "open kbp://home
quit" "Home | kbp://home"

# --- dependable reading and navigation milestone ---------------------------------
run history_restores_absolute_scroll "open $BASE/navigation.html
scroll 500
open $BASE/article.html
scroll 700
back
assert scroll 500 500
fwd
assert scroll 700 700
back
assert scroll 500 500
quit" "[assert] scroll 500 500: PASS" "[assert] scroll 700 700: PASS"

run tabs_preserve_history_and_scroll "open $BASE/navigation.html
scroll 500
open $BASE/article.html
scroll 700
action newtab
assert back false
open $BASE/plain.html
tabstep -1
assert url $BASE/article.html
assert scroll 700 700
assert back true
back
assert url $BASE/navigation.html
assert scroll 500 500
fwd
assert scroll 700 700
quit" "[assert] scroll 500 500: PASS"

run close_active_and_last_tab "open $BASE/navigation.html
action newtab
open $BASE/plain.html
action tabs
tapsel a[href=\"kb:closetab:1\"]
assert tabs 1
assert url $BASE/navigation.html
assert visible a[href=\"kb:closemenu\"]
tapsel a[href=\"kb:closetab:0\"]
assert tabs 1
assert url kbp://home
assert back false
assert visible a[href=\"kb:closemenu\"]
quit" "[assert] url kbp://home: PASS"

run cached_source_matches_document "open $BASE/navigation.html
open $BASE/article.html
back
action width:800
assert title Navigation fixture
text #viewport
quit" "[text] #viewport: width=800;wide=true"

run reader_survives_back_and_reflow "open $BASE/article.html
action reader
open $BASE/plain.html
back
action font:+
count nav
text h1
action reader
count nav
quit" "[count] nav: 0" "[count] nav: 1" "[text] h1: Test article"

run cached_layout_uses_current_settings "open $BASE/navigation.html
text #viewport
open $BASE/article.html
action width:800
back
text #viewport
visible #wide
quit" "[text] #viewport: width=600;wide=false" "[text] #viewport: width=800;wide=true" "[visible] #wide: 1"

run fragment_preserves_dom_and_history "open $BASE/navigation.html
tapsel #edit
tapsel #section-link
assert url $BASE/navigation.html#ch%C6%B0%C6%A1ng+2:reading
assert scroll 2000 6000
text #state
back
assert scroll 0 0
fwd
assert scroll 2000 6000
text #state
quit" "[text] #state: remembered"

# A fragment jump must update location.href as well as location.hash (the prelude copies the
# location fields; href is a getter and needs an explicit copy).
run fragment_updates_location_href "open $BASE/navigation.html
tapsel a[href^=\"#ch\"]
tapsel #read-href
text #href
quit" "[text] #href: $BASE/navigation.html#ch%C6%B0%C6%A1ng+2:reading"

run initial_fragment_and_named_anchor "open $BASE/navigation.html#legacy
assert url $BASE/navigation.html#legacy
assert scroll 2000 6000
open $BASE/navigation.html#
assert scroll 0 0
quit" "[assert] scroll 0 0: PASS"

run missing_fragment_keeps_position "open $BASE/navigation.html
scroll 500
open $BASE/navigation.html#missing
assert scroll 500 500
quit" "[assert] scroll 500 500: PASS"

run query_change_loads_new_document "open $BASE/navigation.html
tapsel #edit
open $BASE/navigation.html?edition=2#legacy
text #state
assert scroll 2000 6000
quit" "[text] #state: original"

run menu_controls_stay_visible "open kbp://home
action menu
scroll 1500
assert visible a[href=\"kb:exit\"]
assert visible a[href=\"kb:closemenu\"]
tapsel a[href=\"kb:exit\"]" "exit requested"

run tab_controls_stay_visible "open kbp://home
action newtab
action newtab
action newtab
action newtab
action newtab
action tabs
scroll 900
assert visible a[href=\"kb:closemenu\"]
tapsel a[href=\"kb:closemenu\"]
action menu
assert visible a[href=\"kb:exit\"]
quit" "[assert] visible a[href=\"kb:exit\"]: PASS"

# --- live sites (opt-in) ----------------------------------------------------------
if [ $NETWORK -eq 1 ]; then
    run hacker_news "open https://news.ycombinator.com/
tapsel .titleline a
quit" "news.ycombinator.com"

    run wikipedia_article "open https://en.wikipedia.org/wiki/E-paper
count h1
quit" "Electronic paper"
fi

echo
echo "passed: $pass   failed: $fail"
[ $fail -eq 0 ] || { echo "failed:$failed"; exit 1; }
exit 0
