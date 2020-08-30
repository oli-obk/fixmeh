#![recursion_limit = "1024"]

use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use typed_html::dom::DOMTree;
use typed_html::{html, text};

fn main() -> std::io::Result<()> {
    const TRIM_TOKENS: &[char] = &['/', '*', ' ', ':', '-', '.', '^', ','];
    let mut dedup: HashMap<_, Vec<_>> = HashMap::new();
    let re = regex::Regex::new(r"[^\\n]*(FIXME|HACK)[^\\n]*").unwrap();
    for file in glob::glob("rust/**/*.rs").expect("glob pattern failed") {
        let filename = file.unwrap();
        let mut text = String::new();
        std::fs::File::open(&filename)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        for cap in re.find_iter(&text) {
            let line_num = text
                .lines()
                .enumerate()
                .find(|(_, s)| unsafe { s.as_ptr().offset_from(text.as_ptr())} > cap.start() as isize)
                .unwrap_or_else(|| panic!("can't find {:?}", cap))
                .0;

            let line = cap.as_str().trim_matches(TRIM_TOKENS).to_owned();
            // trim the leading `rust` part from the path
            let filename: PathBuf = filename.iter().skip(1).collect();
            dedup
                .entry(line)
                .or_default()
                .push((filename.clone(), line_num));
        }
    }
    let mut lines: Vec<_> = dedup.into_iter().collect();
    lines.sort_by(|(a, _), (b, _)| a.cmp(b));
    // sorry, ignoring single and double digit issues
    // We can't depend on a starting `#` either, because some people just use `FIXME 1232`
    let issue_regex = regex::Regex::new(r"[1-9][0-9]{2,}").unwrap();
    let fixme_regex = regex::Regex::new(r"(FIXME|HACK)\(([^\)]+)\)").unwrap();
    let doc: DOMTree<String> = html!(
        <html>
            <head>
                <title>"FIXMEs in the rustc source"</title>
                <style>
                "table, th, td {
                    border: 1px solid black;
                }"
                </style>
            </head>
            <body>
                <table>
                <tr><th>"Description"</th><th>"Source"</th></tr>
                { lines.iter().map(|(text, entries)| {
                    let mut links = linkify::LinkFinder::new();
                    let mut last = 0;
                    let mut clean_text = Vec::new();
                    let bold_names = |clean_text: &mut Vec<_>, text: &str| {
                        if let Some(capture) = fixme_regex.captures(text) {
                            let found = capture.get(2).unwrap();
                            clean_text.push(html!(<span>{text!(&text[..found.start()])}</span>));
                            clean_text.push(html!(<span><strong>{text!(found.as_str())}</strong></span>));
                            clean_text.push(html!(<span>{text!(&text[found.end()..])}</span>));
                        } else {
                            clean_text.push(html!(<span>{text!(text)}</span>));
                        }
                    };
                    let issue_links = |clean_text: &mut Vec<_>, text| {
                        let mut last = 0;
                        for found in issue_regex.find_iter(text) {
                            if found.start() != last {
                                bold_names(clean_text, &text[last..found.start()]);
                            }
                            last = found.end();
                            clean_text.push(html!(<span><a href= { format!("https://github.com/rust-lang/rust/issues/{}", found.as_str())}>{ text!(found.as_str()) }</a></span>));
                        }
                        if last != text.len() {
                            bold_names(clean_text, &text[last..]);
                        }
                    };
                    for link in links.links(text) {
                        // fill in intermediate text
                        if link.start() != last {
                            issue_links(&mut clean_text, &text[last..link.start()]);
                        }
                        last = link.end();
                        let link_text = text!(link.as_str().trim_start_matches("https://").trim_start_matches("github.com/"));
                        clean_text.push(html!(<span><a href={link.as_str()}>{ link_text }</a></span>));
                    }
                    if last != text.len() {
                        issue_links(&mut clean_text, &text[last..]);
                    }
                    html!(
                        <tr>
                        <td>
                            { clean_text }
                        </td>
                        <td>
                            { entries.iter().map(|(file, line)| html!(
                                <a href={ format!("https://github.com/rust-lang/rust/blob/master/{}#L{}", file.display(), line) }>
                                {
                                    let mut file: PathBuf = file.iter().skip(1).collect();
                                    file.set_extension("");
                                    let file = file.display().to_string();
                                    let file = file.trim_start_matches("lib");
                                    text!("{}", file)
                                }<br/>
                                </a>
                            ))}
                        </td>
                        </tr>
                    )
                }) }
                </table>
            </body>
        </html>
    );
    let doc_str = doc.to_string();

    let _ = std::fs::create_dir("build");
    let _ = std::fs::remove_file("build/index.html");
    let mut outfile = std::fs::File::create("build/index.html")?;
    outfile.write_all(doc_str.as_bytes())
}
