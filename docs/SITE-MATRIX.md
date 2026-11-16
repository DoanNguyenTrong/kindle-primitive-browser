# Site matrix

Ten real sites, checked by the desktop build (`scripts/desktop-regression.sh --network`). Each site is
opened and its page title must contain the expected text. This shows that the page loads and the
engine produces the document. It does not test the whole task on the site, and it is not device
evidence: run the same sites on the Oasis before a release.

Results of the run on 2026-11-16 (desktop build, `main` + `feature/diagnostics-ssh`):

| Scenario | Site | Title checked | Result |
|---|---|---|---|
| `hacker_news` | news.ycombinator.com | link text on the front page | pass |
| `wikipedia_article` | en.wikipedia.org/wiki/E-paper | heading `Electronic paper` | pass |
| `matrix_gutenberg_book` | gutenberg.org/ebooks/1342 | `Pride and Prejudice` | **fail**: the server returned `503` at the time of the run, for the book page and its HTML file alike |
| `matrix_python_docs` | docs.python.org (functools) | `functools` | pass |
| `matrix_mdn_javascript` | developer.mozilla.org (JavaScript) | `JavaScript` | pass |
| `matrix_github_repo` | github.com/rust-lang/rust | `rust-lang/rust` | pass |
| `matrix_duckduckgo` | html.duckduckgo.com (query: kindle) | `kindle` | pass |
| `matrix_bbc_news` | bbc.com/news | `BBC` | pass |
| `matrix_lobsters` | lobste.rs | `Lobsters` | pass |
| `matrix_stackoverflow` | stackoverflow.com/questions/11227809 | `processing a sorted array` | **fail**: the site returns `403` to this browser (bot protection). Not worked around: the browser does not pretend to be another browser. |
| `matrix_wikipedia_kindle` | en.wikipedia.org/wiki/Amazon_Kindle | `Kindle` | pass |
| `matrix_hacker_news_front` | news.ycombinator.com | `Hacker News` | pass |

Eight of the ten pass. The two failures are the sites' own responses, not browser errors, and they
will be retried at the next run.

## Not covered yet

- Forms and log-ins on real sites (the local fixtures cover the mechanics).
- Scripts-heavy single-page applications.
- Pages with video or large image sets, on the device.
- Measurements on the Oasis (page turn times, peak memory). See the roadmap.
