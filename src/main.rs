// RUST CLI v1
// What does it do?
// it should act as an interface to gpt api through the CLI
// it should have some reference of what i am looking at and working on
// eventually it should do more than just gpt, maybe embed stuff and RAG

use clap::Parser;
use std::error::Error;
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use log::LevelFilter;
use log::info;
use std::collections::HashMap;
use std::process::Command;
use rusty_tesseract::{Args,Image};

#[derive(Parser)]
struct Cli {
    command: String,
    param: String
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
    content: String,
}

#[derive(Deserialize)]
struct ChatOutput {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

struct WindowInfo {
    id: String,
}

fn main() {
    let args = Cli::parse();

    simple_logging::log_to_file("test.log", LevelFilter::Info).expect("error setting up logger");

    let rt = Runtime::new().unwrap();
    match args.command.as_str() {
        "gpt" => {
            let future = make_gpt_api_call(args.param);
            rt.block_on(future).expect("Error making api call");
        }
        "win" => {
            let ocrd_screenshots = wmctrl("-lx");
            let prompt = format!("These are OCR strings from the windows currently open on my computer {} ",ocrd_screenshots.join(" "));
            let future = make_gpt_api_call(format!("{} {}", prompt, args.param));
            println!("{}", prompt);
            rt.block_on(future).expect("Error making api call");
        }
        _ =>{ println!("unknown command") }
    }
}

async fn make_gpt_api_call(msg: String) -> Result<(), Box<dyn Error>> {

    let token = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    let chat_input = ChatInput {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: msg.clone(),
        }],
        temperature: 0.7,
    };

    let api_url = "https://api.openai.com/v1/chat/completions";

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

    println!("{}", chat_output.choices[0].message.content);

    info!("msg: {}, \n response: {}", msg.clone(), chat_output.choices[0].message.content);
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

        let window = WindowInfo {
            id,
        };

        windows.push(window);
    }

    // requires imagemagick
    // requires xwd
    let mut outputs: Vec<String> = Vec::new();

    for window in &windows {
        Command::new("sh")
            .arg("-c")
            .arg(format!("xwd -id {} -out ./tmp/{}.xwd", window.id, window.id))
            .output()
            .expect("xwd failed");

       let convert_output = Command::new("sh")
            .arg("-c")
            .arg(format!("convert ./tmp/{}.xwd -alpha off -negate  ./tmp/{}.png", window.id, window.id)) //-negate
            .output()
            .expect("image magick failed");
        if convert_output.status.success(){
            let ocr = read_image(window.id.clone());
            if ocr.len() > 20 {
                outputs.push(ocr);
            }
        }
    }
    return outputs;
}

fn read_image(w_id: String) -> String {
    let img = Image::from_path(format!("./tmp/{}.png", w_id)).expect("unable to read image");
    let my_args = Args {
        lang: "eng".to_string(),
        config_variables: HashMap::from([(
            "tessedit_char_whitelist".into(),"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ ".into(),
            )]),
        dpi: Some(300),
        psm: Some(6),
        oem: Some(3)
    };
    let output = rusty_tesseract::image_to_string(&img, &my_args).unwrap();
    return output;
}
