// RUST CLI v1
// What does it do?
// it should act as an interface to gpt api through the CLI
// it should have some reference of what i am looking at and working on
// eventually it should do more than just gpt, maybe embed stuff and RAG

use base64::encode;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::{read_dir, remove_file, File};
use std::io::Read;
use std::process::Command;
use tokio::runtime::Runtime;

#[derive(Parser)]
struct Cli {
    command: String,
    param: String,
}

#[derive(Serialize)]
struct ChatInput {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: Vec<ContentItem>,
}

#[derive(Serialize, Deserialize)]
struct MessageOut {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct ChatOutput {
    choices: Vec<ChoiceOut>,
}

#[derive(Serialize, Deserialize)]
struct ChoiceOut {
    message: MessageOut,
}

struct WindowInfo {
    id: String,
}

#[derive(Serialize, Deserialize)]
struct ContentItem {
    #[serde(rename = "type")]
    content_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<ImageUrl>,
}

#[derive(Serialize, Deserialize)]
struct ImageUrl {
    url: String,
}

fn main() {
    let args = Cli::parse();

    let rt = Runtime::new().unwrap();
    match args.command.as_str() {
        "gpt" => {
            let future = make_gpt_api_call(args.param, None);
            rt.block_on(future).expect("Error making api call");
        }
        "win" => {
            let b64_screenshots = wmctrl("-lx");
            let future = make_gpt_api_call(format!("{}", args.param), Some(b64_screenshots));
            rt.block_on(future).expect("Error making api call");
        }
        _ => {
            println!("unknown command")
        }
    }
    cleanup_tmp_dir();
}

async fn make_gpt_api_call(
    msg: String,
    b64_screenshots: Option<Vec<String>>,
) -> Result<(), Box<dyn Error>> {
    let token = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let mut content_items = vec![ContentItem {
        content_type: "text".to_string(),
        text: Some(msg),
        image_url: None,
    }];

    if let Some(screenshots) = b64_screenshots {
        for screenshot in screenshots {
            let image_url_item = ContentItem {
                content_type: "image_url".to_string(),
                text: None,
                image_url: Some(ImageUrl {
                    url: format!("data:image/jpeg;base64,{}", screenshot),
                }),
            };
            content_items.push(image_url_item);
        }
    }
    let chat_input = ChatInput {
        model: "gpt-4-vision-preview".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: content_items,
        }],
        temperature: 0.7,
    };

    let api_url = "https://api.openai.com/v1/chat/completions";
    // println!("\n\nF\n {}\n\n", json_string);
    let client = reqwest::Client::new();
    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&chat_input)
        .send()
        .await?;

    let text = response.text().await?;
    let chat_output: ChatOutput = serde_json::from_str(&text).expect(&text);

    let response_string = serde_json::to_string_pretty(&chat_output).unwrap();
    println!("{}", response_string);

    Ok(())
}

fn wmctrl(args: &str) -> Vec<String> {
    // requires wmctrl
    let wmctrl_cmd_output = Command::new("sh")
        .arg("-c")
        .arg(format!("wmctrl {}", args))
        .output()
        .unwrap_or_else(|_| panic!("failed to execute 'wmctrl {}'", args));
    let stdout = String::from_utf8(wmctrl_cmd_output.stdout).expect("Not UTF8");
    let lines: Vec<&str> = stdout.lines().collect();

    let mut windows: Vec<WindowInfo> = Vec::new();
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let id = parts[0].to_string();

        let window = WindowInfo { id };

        windows.push(window);
    }

    // requires imagemagick
    // requires xwd
    let mut outputs: Vec<String> = Vec::new();

    for window in &windows {
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "xwd -id {} -out ./tmp/{}.xwd",
                window.id, window.id
            ))
            .output()
            .expect("xwd failed");

        let convert_output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "convert ./tmp/{}.xwd -alpha off -negate  ./tmp/{}.png",
                window.id, window.id
            )) //-negate
            .output()
            .expect("image magick failed");
        if convert_output.status.success() {
            let b64string = base_64_image(window.id.clone());
            outputs.push(b64string);
        }
    }
    return outputs;
}

fn base_64_image(w_id: String) -> String {
    let mut file = File::open(format!("./tmp/{}.png", w_id)).expect("Unable to read image");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .expect("Error loading image into buffer");
    return encode(&buffer);
}

fn cleanup_tmp_dir() {
    let path = "./tmp";
    let entries = read_dir(path).unwrap();
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            remove_file(path).unwrap();
        }
    }
}
