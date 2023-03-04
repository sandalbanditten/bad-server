#![feature(nonzero_min_max)]

use std::error::Error;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::{env, fs, process};

use backend::{log, ThreadPool};

fn main() -> Result<(), Box<dyn Error>> {
    log("Starting server", false, 0);

    if env::current_dir()? != PathBuf::from("/home/notroot/Code/Rust/backend/www") {
        log("Start the server from www", true, 0);
        process::exit(1);
    }
    let ip = "127.0.0.1";
    // let ip = "192.168.251.51";
    let port = "7878";
    let listener = TcpListener::bind(format!("{ip}:{port}"))?;

    let mut connections = 0;

    let log_msg = format!("Binding to {ip} at port {port}");
    log(&log_msg, false, 0);

    let pool_size = match NonZeroUsize::new(12) {
        Some(pool_size) => pool_size,
        None => {
            log("Impossible pool size, fallback to 1", true, 0);
            NonZeroUsize::MIN
        }
    };
    let pool = ThreadPool::new(pool_size);

    for stream in listener.incoming() {
        let stream = stream?;

        connections += 1;
        pool.execute(
            move || handle_connection(stream, connections).unwrap(),
            &connections,
        )?;
    }

    log("Shutting down", false, 0);

    Ok(())
}

fn handle_connection(mut stream: TcpStream, jobnum: usize) -> Result<(), Box<dyn Error>> {
    let log_msg = format!("Connection established: {stream:#?}");
    log(&log_msg, false, jobnum);

    let mut buf = vec![0; 1024];
    stream.read_to_end(&mut buf)?;

    let str = String::from_utf8_lossy(&buf);
    // println!("Buffer contains:\n{str}");

    let (file_name, status_line) = if buf.starts_with(b"GET") {
        let file_name = str
            .split_whitespace()
            .nth(1)
            .expect("HTTP should have a second word in the header");

        let file_name = file_name
            .chars()
            .take_while(|&c| c != '#' && c != '?')
            .collect::<String>();

        // when in doubt, leak memory
        let file_name = Box::leak(Box::new(file_name));

        let file_name = match file_name.as_str() {
            "/" => "index.html",
            file => &file[1..], // Skip the first character, a '/'
        };

        (file_name, "HTTP/1.1 200 OK")
    } else {
        ("404.html", "HTTP/1.1 404 NOT FOUND")
    };

    let log_msg = format!("Sending {file_name}");
    log(&log_msg, false, jobnum);

    let response = file_to_http_bytes(file_name, status_line, jobnum)?;

    stream.write_all(&response)?;
    stream.flush()?;
    log("Wrote response", false, jobnum);

    buf.clear();

    log("Ended connection", false, jobnum);

    Ok(())
}

fn file_to_http_bytes(
    file_name: &str,
    status_line: &str,
    jobnum: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut contents = match fs::read(file_name.to_lowercase()) {
        Ok(f) => f,
        Err(_) => {
            let log_msg = format!("File {file_name} not found, sending 404 instead");
            log(&log_msg, true, jobnum);
            fs::read("404.html")?
        }
    };

    if file_name.contains("home") || file_name.contains("..") {
        contents = fs::read("404.html")?;
    }

    let len = contents.len();
    let content_length = format!("Content-Length: {len}");
    let mut response = Vec::from(status_line.as_bytes());

    response.append(&mut Vec::from("\r\n".as_bytes()));
    response.append(&mut Vec::from(content_length.as_bytes()));
    response.append(&mut Vec::from("\r\n\r\n".as_bytes()));
    response.append(&mut contents);

    Ok(response)
}
