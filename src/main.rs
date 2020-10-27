use clap::{Arg, App};
use std::fs::*;
use wkhtmltopdf::*;
use std::io::{Read, Write};
use std::path::{PathBuf, Path};
use std::str::FromStr;
use crate::parser::{parse, relink, parse_theme};
use crate::non_md::handle_non_md;

mod parser;
mod non_md;

const HEADERS: &str = "<script src=\"//cdnjs.cloudflare.com/ajax/libs/highlight.js/10.1.2/highlight.min.js\"></script>\n<script>hljs.initHighlightingOnLoad();</script>";

fn main() {
    let mut pdf_app = PdfApplication::new().expect("Failed to init PDF application");
    let matches = App::new("Iridium")
        .version("0.3")
        .author("Thomas B. <tom.b.2k2@gmail.com>")
        .about("A static site generator for the modern era.")
        .arg(Arg::with_name("in")
            .short("i")
            .long("input")
            .value_name("PATH")
            .help("Sets the location to read from. (Can be a file or directory)")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("out")
            .short("o")
            .long("output")
            .value_name("PATH")
            .help("Sets the location to write to. (Must be a directory)")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("watermark")
            .long("no-water-mark")
            .help("Removes the watermark")
            .takes_value(false))
        .arg(Arg::with_name("pdf")
            .long("pdf")
            .help("Renders the output as PDF content, instead of the normal HTML")
            .takes_value(false))
        .arg(Arg::with_name("pdf-mirror")
            .long("pdf-mirror")
            .help("Renders the output as PDF content, as well as the normal HTML")
            .takes_value(false))
        .arg(Arg::with_name("theme")
            .long("theme")
            .short("t")
            .help("Selects the theme to use in the rendering process (Defualt: 'Iridium')")
            .takes_value(true))
        .get_matches();

    let mut theme = "Iridium";
    if matches.is_present("theme") {
        theme = matches.value_of("theme").unwrap();
    }

    let mut wm = true;
    if matches.is_present("watermark") {
        wm = false;
    }

    let mut pdf = false;
    if matches.is_present("pdf") {
        pdf = true;
    }

    let mut pdfm = false;
    if matches.is_present("pdf-mirror") {
        pdfm = true;
    }

    if pdf && !pdfm {
        println!("PDF Mode")
    } else if !pdf && pdfm {
        println!("PDF Mirror Mode")
    } else if pdf && pdfm {
        pdf = false;
        println!("PDF Mirror Mode")
    } else {
        println!("HTML Mode")
    }

    if pdf || pdfm {
        wm = false;
    }

    if matches.is_present("in") && matches.is_present("out") {
        let input = matches.value_of("in").unwrap();
        let output = matches.value_of("out").unwrap();
        let p = Path::new(input).canonicalize();
        if p.is_ok() {
            let pa = p.unwrap();
            println!("Canonicalized {:#?}", pa);
            let metadata = metadata(pa);
            if metadata.is_ok() {
                let md = metadata.unwrap();
                if md.is_dir() {
                    println!("Discovering Files...");
                    let paths = read_directory(input);
                    let tot = paths.len();
                    println!("Discovered {} Files", tot);
                    println!("Migrating incompatible files...");
                    let processes = handle_non_md(paths, input, output);
                    let mut ptot = processes.len();
                    if pdf || pdfm {
                        ptot = ptot * 2
                    }
                    let mut index = tot - ptot;

                    for (source, destination) in processes {
                        read_file(source, destination, wm, pdf, pdfm, &mut pdf_app, theme);
                        index += 1;
                        if pdf || pdfm {
                            index += 1;
                        }
                    }
                    println!("Migrated {} Files", index);
                    println!("Compiled {} Files", ptot);
                } else {
                    let mut nodes = input.clone().split("/").collect::<Vec<&str>>();
                    let file = nodes.pop();
                    let mut root: String = nodes.join("/");

                    let mut destination = String::from(output);
                    if !destination.ends_with("/") {
                        destination = format!("{}/", destination)
                    }

                    let pathstr = input.clone();
                    let p2 = pathstr.clone();
                    let mut tiers = p2.split(root.as_str()).collect::<Vec<&str>>();
                    let final_path = format!("{}{}", destination, tiers.pop().unwrap());
                    read_file(pathstr.to_string(), final_path, wm, pdf, pdfm, &mut pdf_app, theme);
                }
                println!("Compilation complete.");
            } else {
                println!("An error occurred (step 1): {:#?}", metadata.unwrap_err());
                std::process::exit(1);
            }
        } else {
            println!("Failed to canonicalize {}\n{:#?}", input, p.unwrap_err())
        }
    }
}

fn read_directory(path: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let files = read_dir(path).unwrap();
    for f in files {
        if f.is_ok() {
            let file = f.unwrap();
            if file.metadata().unwrap().is_dir() {
                let mut p1 = read_directory(file.path().to_str().unwrap());
                paths.append(&mut p1);
            } else {
                paths.push(file.path())
            }
        } else {
            println!("Unable to open: {:#?}", f.unwrap_err())
        }
    }
    paths
}

fn read_file(path: String, out_path: String, wm: bool, pdf: bool, pdfm: bool, pdf_app: &mut PdfApplication, theme: &str) {
    // Read the file
    let mut content_file = File::open(&path).unwrap();
    let mut content = String::new();
    content_file.read_to_string(&mut content).unwrap();

    // Read the theme's stylesheet
    let mut css: &str = parse_theme(theme);

    let mut watermark = String::from("<div style=\"text-align: center; padding: 1em; color: #aaa\"><h4>Powered by <a href=\"https://github.com/fatalcenturion/Iridium\">Iridium</a><h4></div>");
    if !wm {
        watermark = String::new();
    }
    // Begin parsing MD to HTML
    let mut html = format!("<!DOCTYPE html>\n<html>\n<head>\n{}\n<style>{}</style>", HEADERS, css);
    html = format!("{}\n<script src=\"https://ajax.googleapis.com/ajax/libs/jquery/3.5.1/jquery.min.js\"></script>\n</head>\n<body>\n<div class=\"container\">", html);
    html = format!("{}\n{}\n</div>\n{}\n<script>{}</script></body>\n<html>", html, parse(content), watermark, "
    document.addEventListener('DOMContentLoaded', function() {
	    document.querySelectorAll(\"h1, h2, h3, h4, h5, h6\").forEach(element => {
		    element.innerHTML += `<div class=\"anchor\" style=\"display:none;\" id=\"${(element.innerText.toLowerCase().replace(/[^\\w]/gmi, \"\")).split(\" \").join(\"-\")}\">Anchor point</div>`;
		    element.children[0].onclick = () => {let em = document.getElementById(window.location.hash.split(/#|\\?[^\\s]*/g).join(\"\")); console.log(`Navigating to: ${window.location.hash.split(/#|\\?[^\\s]*/g).join(\"\")}`); if(em !== null && em !== undefined) em.parentElement.scrollIntoView({ behavior: 'instant', block: 'start' })}
		    })
	let em = document.getElementById(window.location.hash.replace(/#|\\?[^\\s]*/g, \"\"));
	console.log(`Navigating to: ${window.location.hash.replace(/#|\\?[^\\s]*/g, \"\")}`);
	if(em !== null && em !== undefined) em.parentElement.scrollIntoView({ behavior: 'instant', block: 'start' })
}, false);");

    let ht2 = html.clone();
    let pdfc = relink(ht2, "pdf");
    html = relink(html, "html");

    let mut out: File;
    let meta = metadata(&out_path);
    let op = out_path.clone();
    let mut entries = op.split("/").collect::<Vec<&str>>();
    let name = entries.pop().unwrap().replace(".md", "").replace(".markdown", "");
    let dir_path = entries.join("/");
    if meta.is_ok() {
        let trydel = remove_file(&out_path);
        if trydel.is_ok() {
            if pdf {
                println!("Compiled: {} (PDF)", &out_path.replace(".md", ".pdf").replace(".markdown", ".pdf"));
                let mut pdf_content = pdf_app.builder()
                    .orientation(Orientation::Landscape)
                    .title(&*name)
                    .margin(Size::Millimeters(0))
                    .build_from_html(&pdfc)
                    .expect("failed to build pdf");
                pdf_content.save(&out_path.replace(".md", ".pdf").replace(".markdown", ".pdf")).expect("failed to save foo.pdf");
            } else if pdfm {
                println!("Compiled: {} (PDF)", &out_path.replace(".md", ".pdf").replace(".markdown", ".pdf"));
                let mut pdf_content = pdf_app.builder()
                    .orientation(Orientation::Landscape)
                    .title(&*name)
                    .margin(Size::Millimeters(0))
                    .build_from_html(&pdfc)
                    .expect("failed to build pdf");
                pdf_content.save(&out_path.replace(".md", ".pdf").replace(".markdown", ".pdf")).expect("failed to save foo.pdf");
                println!("Compiled: {} (HTML)", &out_path.replace(".md", ".html").replace(".markdown", ".html"));
                out = File::create(&out_path.replace(".md", ".html").replace(".markdown", ".html")).unwrap();
                write(html, &mut out);
            } else {
                println!("Compiled: {} (HTML)", &out_path.replace(".md", ".html").replace(".markdown", ".html"));
                out = File::create(&out_path.replace(".md", ".html").replace(".markdown", ".html")).unwrap();
                write(html, &mut out)
            }
        } else {
            println!("Failed to get metadata for \"{}\".\nReason: {:#?}", out_path, trydel.unwrap_err());
        }
    } else {
        create_dir_all(dir_path);
        if pdf {
            println!("Compiled: {} (PDF)", &out_path.replace(".md", ".pdf").replace(".markdown", ".pdf"));
            let mut pdf_content = pdf_app.builder()
                .orientation(Orientation::Landscape)
                .title(&*name)
                .margin(Size::Millimeters(0))
                .build_from_html(&pdfc)
                .expect("failed to build pdf");
            pdf_content.save(&out_path.replace(".md", ".pdf").replace(".markdown", ".pdf")).expect("failed to save foo.pdf");
        } else if pdfm {
            println!("Compiled: {} (PDF)", &out_path.replace(".md", ".pdf").replace(".markdown", ".pdf"));
            let mut pdf_content = pdf_app.builder()
                .orientation(Orientation::Landscape)
                .title(&*name)
                .margin(Size::Millimeters(0))
                .build_from_html(&pdfc)
                .expect("failed to build pdf");
            pdf_content.save(&out_path.replace(".md", ".pdf").replace(".markdown", ".pdf")).expect("failed to save foo.pdf");
            println!("Compiled: {} (HTML)", &out_path.replace(".md", ".html").replace(".markdown", ".html"));
            out = File::create(&out_path.replace(".md", ".html").replace(".markdown", ".html")).unwrap();
            write(html, &mut out);
        } else {
            println!("Compiled: {} (HTML)", &out_path.replace(".md", ".html").replace(".markdown", ".html"));
            out = File::create(&out_path.replace(".md", ".html").replace(".markdown", ".html")).unwrap();
            write(html, &mut out)
        }
    }
}

fn write(content: String, file: &mut File) {
    file.write(content.as_bytes());
}

