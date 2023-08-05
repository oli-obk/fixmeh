#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

use maud::{html, Markup, PreEscaped, Render};

fn into_markup<T>(x: T) -> Markup
where
    T: IntoIterator,
    T::Item: Render,
{
    let mut s = String::new();
    for item in x {
        item.render_to(&mut s);
    }
    PreEscaped(s)
}

fn main() -> std::io::Result<()> {
    const TRIM_TOKENS: &[char] = &['/', '*', ' ', ':', '-', '.', '^', ','];
    let mut dedup: HashMap<_, Vec<_>> = HashMap::new();

    for file in glob::glob("rust/**/*.rs").expect("glob pattern failed") {
        let filename = file.unwrap();
        let mut text = String::new();
        if let Err(e) = std::fs::File::open(&filename)
            .unwrap()
            .read_to_string(&mut text)
        {
            eprintln!("skipping {:?}: {}", filename, e);
            continue;
        }

        for (line_num, line) in text.lines().enumerate() {
            if !line.contains("FIXME") && !line.contains("HACK") {
                continue;
            }

            let line = line.trim_matches(TRIM_TOKENS).to_owned();
            let filename: PathBuf = filename.iter().skip(1).collect();
            dedup
                .entry(line)
                .or_default()
                .push((filename.clone(), line_num + 1));
        }
    }
    let mut lines: Vec<_> = dedup.into_iter().collect();
    lines.sort_by(|(a, _), (b, _)| a.cmp(b));
    let fixme_regex = regex::Regex::new(r"(FIXME|HACK)\(([^\)]+)\)").unwrap();

    let mut querier = IssueQuerier::new();

    let doc: maud::Markup = html!(
        html {
            head {
                title {
                    "FIXMEs in the rustc source"
                }
                style {
                    "table, th, td {
                        border: 1px solid black;
                    }"
                }
            }
            body {
                table {
                    tr { th { "Description" } th { "Source" } th { "Issue states" } }
                    (into_markup(lines.iter().map(|(text, entries)| {
                        let links = linkify::LinkFinder::new();
                        let mut last = 0;
                        let mut clean_text = Vec::new();
                        let bold_names = |clean_text: &mut Vec<_>, text: &str| {
                            if let Some(capture) = fixme_regex.captures(text) {
                                let found = capture.get(2).unwrap();
                                clean_text.push(html!(span {(&text[..found.start()])}));
                                clean_text.push(html!(span { strong { (found.as_str()) } }));
                                clean_text.push(html!(span { (&text[found.end()..]) }));
                            } else {
                                clean_text.push(html!(span { (text) }));
                            }
                        };
                        let mut issue_links = |clean_text: &mut Vec<_>, issue_states: &mut String, text| {
                            let mut last = 0;
                            for found in issue_references(text) {
                                if found.start != last {
                                    bold_names(clean_text, &text[last..found.start]);
                                }
                                last = found.end;
                                let found_str = &text[found.start..found.end];
                                if let Ok(issue_nbr) = u64::from_str_radix(found_str, 10) {
                                    *issue_states += &querier.issue_state(issue_nbr);
                                    *issue_states += " "; // Trailing spaces in HTML are ignored, so this is fine.
                                }
                                clean_text.push(html!(span { a href=(format!("https://github.com/rust-lang/rust/issues/{}", found_str)) { (found_str) } }));
                            }
                            if last != text.len() {
                                bold_names(clean_text, &text[last..]);
                            }
                        };
                        let mut issue_states = String::new();
                        for link in links.links(text) {
                            // fill in intermediate text
                            if link.start() != last {
                                issue_links(&mut clean_text, &mut issue_states, &text[last..link.start()]);
                            }
                            last = link.end();
                            let link_text = link.as_str().trim_start_matches("https://").trim_start_matches("github.com/");
                            clean_text.push(html!( span { a href=(link.as_str()) { (link_text) } } ));
                        }
                        if last != text.len() {
                            issue_links(&mut clean_text, &mut issue_states, &text[last..]);
                        }
                        html!(
                            tr {
                                td {
                                    (into_markup(clean_text))
                                }
                                td {
                                    (into_markup(entries.iter().map(|(file, line)| html!(
                                        a href=(format!("https://github.com/rust-lang/rust/blob/master/{}#L{}", file.display(), line)) {
                                            ({
                                                let mut file: PathBuf = file.iter().skip(1).collect();
                                                file.set_extension("");
                                                let file = file.display().to_string();
                                                let file = file.trim_start_matches("lib");
                                                file.to_owned()
                                            })
                                        }
                                        br;
                                    ))))
                                }
                                td {
                                    (issue_states)
                                }
                            }
                        )
                    })))
                }
                script {
                    (PreEscaped(
                    "
                    // Copied Verbatim from https://stackoverflow.com/a/49041392.
                    const getCellValue = (tr, idx) => tr.children[idx].innerText || tr.children[idx].textContent;
    
                    const comparer = (idx, asc) => (a, b) => ((v1, v2) => 
                        v1 !== '' && v2 !== '' && !isNaN(v1) && !isNaN(v2) ? v1 - v2 : v1.toString().localeCompare(v2)
                        )(getCellValue(asc ? a : b, idx), getCellValue(asc ? b : a, idx));
    
                    // do the work...
                    document.querySelectorAll('th').forEach(th => th.addEventListener('click', (() => {
                        const table = th.closest('table');
                        Array.from(table.querySelectorAll('tr:nth-child(n+2)'))
                            .sort(comparer(Array.from(th.parentNode.children).indexOf(th), this.asc = !this.asc))
                            .forEach(tr => table.appendChild(tr) );
                    })));
                    "))
                }
            }
        }
    );
    let doc_str = doc.into_string();

    let _ = std::fs::create_dir("build");
    let _ = std::fs::remove_file("build/index.html");
    let mut outfile = std::fs::File::create("build/index.html")?;
    outfile.write_all(doc_str.as_bytes())
}

#[derive(Debug, PartialEq, Eq)]
struct IssueReference {
    start: usize,
    end: usize,
}

/// Given a string, return a list of start and end indices to what looks like
/// issue references.
fn issue_references(text: &str) -> Vec<IssueReference> {
    // sorry, ignoring single and double digit issues
    // We can't depend on a starting `#` either, because some people just use `FIXME 1232`
    let issue_regex = regex::Regex::new(r"\b([1-9][0-9]{2,})([^%a-zA-Z]|$)").unwrap();

    issue_regex
        .captures_iter(text)
        .map(|m| IssueReference {
            start: m.get(1).unwrap().start(),
            end: m.get(1).unwrap().end(),
        })
        .collect()
}

struct IssueQuerier {
    runtime: tokio::runtime::Runtime,
    octocrab: octocrab::Octocrab,
    cache: HashMap<u64, String>,
}

impl IssueQuerier {
    fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // octocrab needs to be initialized in async context in order to
        // function correctly
        let octocrab = runtime.block_on(async {
            octocrab::Octocrab::builder()
                .personal_token(std::env::var("GITHUB_TOKEN").expect("go to https://github.com/settings/tokens?type=beta and generate a token that can read public repos"))
                .build()
                .unwrap()
        });

        IssueQuerier {
            runtime,
            octocrab,
            cache: HashMap::new(),
        }
    }
    fn issue_state(&mut self, issue_nbr: u64) -> String {
        self.cache
            .entry(issue_nbr)
            .or_insert_with(|| {
                // Do this serially to not make the GitHub API rate limiter
                // nervous. It is pretty slow, but that's what we want.
                self.runtime.block_on(async {
                    eprintln!("HTTP GET issue state for {issue_nbr}");
                    self.octocrab
                        .issues("rust-lang", "rust")
                        .get(issue_nbr)
                        .await
                        .map(|i| format!("{:?}", i.state))
                        .unwrap_or_else(|e| {
                            let error = format!("error: {}", e);
                            eprintln!("{}", error);
                            String::new() // Hide errors from users
                        })
                })
            })
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_references() {
        let cases = [
            (
                "FIXME(jackh726): This is a hack. It's somewhat like",
                vec![]
            ),
            (
                "// FIXME: not 100% sure why these crop up, but return an empty tree to avoid a panic",
                vec![]
            ),
            (
                "// FIXME(mu001999) E0599 maybe not suitable here because it is for types",
                vec![],
            ),
            (
                "FIXME implement 128bit atomics",
                vec![],
            ),
            (
                "FIXME: #7698, false positive of the internal lints",
                vec![IssueReference {start: 8, end: 12}],
            ),
            (
                "FIXME: 91167",
                vec![IssueReference {start:7, end: 12}]
            ),
            (
                "ignore-android: FIXME (#20004)",
                vec![IssueReference {start:24, end: 29}]
            ),
            (
                "ignore-android: FIXME(#10381)",
                vec![IssueReference {start:23, end: 28}]
            ),
            (
                "frame_pointer: FramePointer::Always, // FIXME 43575: should be MayOmit",
                vec![IssueReference {start:46, end: 51}]
            ),
            (
                "FIXME: Report diagnostic on 404",
                vec![IssueReference {start:28, end: 31}] // TODO: Fix false positive
            ),
            (
                "FIXME: [0..200; 2];",
                vec![IssueReference {start: 11, end: 14}], // TODO: Fix false positive
            ),
            (
                "FIXME(bytecodealliance/wasmtime#6104) use bitcast instead of store to get from i64x2 to i128",
                vec![IssueReference {start: 32, end: 36}], // TODO: Link to the correct repo
            ),
            (
                "#[allow(dead_code)] // FIXME(81658): should be used + lint reinstated after #83171 relands",
                vec![IssueReference {start:29, end: 34}, IssueReference {start:77, end: 82}]
            ),
        ];

        for case in cases {
            let text = case.0;
            let expected = case.1;

            assert_eq!(issue_references(text), *expected, "{text}");
        }
    }
}
