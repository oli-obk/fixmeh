#![recursion_limit = "1024"]

use std::collections::HashMap;
use std::io::Write;
use typed_html::dom::DOMTree;
use typed_html::{html, text};

fn main() -> std::io::Result<()> {
    let mut dedup: HashMap<_, Vec<_>> = HashMap::new();
    for line in include_str!("../fixmes.txt").lines() {
        let mut line = line.splitn(3, ':');
        let filename = line.next().unwrap();
        let line_num = line.next().unwrap();
        // take everything after the `FIXME`
        let line = line.next().unwrap();
        let text = line.splitn(2, "FIXME").nth(1).unwrap();
        let text = text.trim().trim_start_matches(':').trim();
        dedup
            .entry(text)
            .or_default()
            .push((filename, line_num, line));
    }
    let mut lines: Vec<_> = dedup.into_iter().collect();
    lines.sort_by_key(|(text, _)| *text);
    let issue_regex = regex::Regex::new(r"#[0-9]+").unwrap();
    let doc: DOMTree<String> = html!(
        <html>
            <head>
                <title>"FIXMEs in the rustc source"</title>
            </head>
            <body>
                <table>
                <tr><th>"Description"</th><th>"Issue"</th><th>"Full text and link to file"</th></tr>
                { lines.iter().map(|(text, entries)| {
                    let mut parser = rfind_url::Parser::new();
                    let url = text.chars().rev().enumerate().filter_map(|(i, c)| parser.advance(c).map(|n| (i, n))).next();
                    let (text, url) = match url {
                        Some((i, n)) => (
                            format!("{}{}", &text[..(text.len() - i - 1)], &text[text.len() - i - 1 + n as usize ..]),
                            Some(&text[text.len() - i - 1 ..][..n as usize]),
                        ),
                        None => (text.to_string(), None),
                    };
                    html!(
                        <tr>
                        <td>
                            { text!(text.clone()) }
                        </td>
                        <td>
                            {
                                if let Some(url) = url {
                                    html!(<a href={url.to_string()}>{ text!("{}", url) }</a>)
                                } else if let Some(found) = issue_regex.find(&text) {
                                    let found = found.as_str();
                                    html!(<a href= { format!("https://github.com/rust-lang/rust/issues/{}", found)}>{ text!(found) }</a>)
                                } else {
                                    html!(<a href="">"no issue link"</a>)
                                }
                            }
                        </td>
                        <td>
                            { entries.iter().map(|(file, line, text)| html!(
                                <a href={ format!("https://github.com/rust-lang/rust/blob/master/{}#L{}", file, line) }>
                                {
                                    let text = text
                                        .trim()
                                        .trim_start_matches('/')
                                        .trim()
                                        .trim_start_matches('*')
                                        .trim()
                                        .trim_start_matches('(')
                                        .trim()
                                        .trim_start_matches("FIXME")
                                        .trim()
                                        .trim_start_matches('^')
                                        .trim()
                                        .trim_start_matches(':')
                                        .trim()
                                        .trim_start_matches('-')
                                        .trim()
                                        .trim_start_matches('.')
                                        .trim();
                                    let text = if text.is_empty() {
                                        format!("{}:{}", file, line)
                                    } else {
                                        text.to_string()
                                    };
                                    text!("{}", text)
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
