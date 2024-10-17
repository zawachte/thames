use std::fs::File;
use std::io::Write;
use std::io;
use tokio::task;
use clap::Parser;

/// A multi-part downloader.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Target URL to download
    #[arg(short, long)]
    url: String,

    /// Number of parts/threads to use for the download
    #[arg(short, long, default_value_t = 10)]
    parts: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let link = args.url;
    let number_of_parts = args.parts as i32;

    let client = reqwest::Client::new();
    // parse the url
    let url = reqwest::Url::parse(&link).unwrap();
    // Get the filename from the URL
    let filename = url.path_segments()
        .map(|segments| segments.last().unwrap().to_string())
        .unwrap();
    println!("Downloading {}", filename);

    let content_length_int = get_file_information(client.clone(), link.clone()).await?;
    let part_size = content_length_int / number_of_parts;
    println!("Size of each part: {}", part_size);

    download(part_size, number_of_parts, filename.clone(), client.clone(), link).await?;
    merge_tmp_files(filename.clone(), number_of_parts).await?;

    println!("Download of {} complete", filename);

    Ok(())
}

async fn download(
    part_size: i32,
    number_of_parts: i32,
    filename: String,
    client: reqwest::Client,
    link: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut handles = vec![];
    for i in 0..number_of_parts {        
        let marker = i as i32;
        let start = marker * part_size;
        let end = (marker + 1) * part_size - 1;
        let part = format!("bytes={}-{}", start, end);
        let client = client.clone();
        let tmp_file_name = format!("{}.{}", filename.clone(), marker);
        let target_link = link.clone();

        let handle = task::spawn(async move {
            println!("Thread {} downloading part: {}", marker, part);

            // Perform a GET request
            let resp = client.get(target_link)
                .header("Range", part)
                .send()
                .await
                .unwrap();

            // Check if the request was successful
            if resp.status().is_success() {
                // Create a file to write the response body to
                let file = File::create(tmp_file_name);
                file.unwrap().write_all(&resp.bytes().await.unwrap()).unwrap();
            } else {
                println!("Request failed with status: {}", resp.status());
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
    Ok(())
}

async fn merge_tmp_files(filename: String, number_of_parts: i32) -> Result<(), Box<dyn std::error::Error>> {
    // Merge the parts
    let mut file = File::create(filename.clone()).unwrap();
    for i in 0..number_of_parts {
        let tmp_file_name = format!("{}.{}", filename, i);
        println!("Merging {} into {}", tmp_file_name, filename);

        let mut tmp_file = File::open(tmp_file_name.clone()).unwrap();
        io::copy(&mut tmp_file, &mut file).unwrap();

        // Delete the temporary file
        std::fs::remove_file(tmp_file_name).unwrap();
    }

    Ok(())
}

async fn get_file_information(client: reqwest::Client, link: String) -> Result<i32, Box<dyn std::error::Error>> {
    // Perform a HEAD request
    let response = client.head(link).send().await?;

    // Print the status code
    println!("Status Code: {}", response.status());

    // Check for 4xx or 5xx status codes
    if response.status().is_client_error() || response.status().is_server_error() {
        return Err(format!("Request failed with status: {}", response.status()).into());
    }

    // Print the headers
    for (key, value) in response.headers() {
        println!("{}: {:?}", key, value);
    }
    
    let content_length = response.headers().get("content-length")
        .ok_or("content-length header not found")?;

    // Convert content_length to integer
    let content_length_int = content_length.to_str()?.parse::<i32>()?;
    // Return the content length
    Ok(content_length_int)
}