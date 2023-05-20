use serde_json::Value;
use std::env;
use std::fs;
    
// Use Bing Search API to answer questions
async fn bing_search(question: &str) -> Option<String> {
    let api_key = env::var("BING_SEARCH_API_KEY")?;
    let url = format!("https://api.bing.microsoft.com/a66fa414-3d6b-4925-88e1-21dec2c33a00/bing/v7.0/search?q={}&mkt=en-us&form=WNSG1", question);
    let resp = reqwest::get(&url).await?;
    let data: Value = resp.json().await?;
    data["webPages"]["value"][0]["snippet"].as_str().map(String::from)
}

// Tools that can be used to answer questions
struct Tools {
    search: (fn(String) -> Option<String>, &'static str),
    calculator: (fn(String) -> String, &'static str), 
}
    
let tools = Tools {
        search: (bing_search, "a search engine. useful for answering questions about current events. input should be a search query."), 
        calculator: (|input| eval::eval(&input).to_string(), 
                     "Useful for getting the result of a mathematical expression. The input to this tool \
                     should be a valid mathematical expression that could be executed by a scientific \
                     calculator.")

// Use GPT-3.5 to complete prompts
async fn complete_prompt(prompt: String, openai_key: &str) -> String {
    let url = "https://api.openai.com/v1/completions";
    let body = serde_json::json!({
        "model": "text-davinci-003",
        "prompt": prompt,
        "max_tokens": 256,
        "temperature": 0.7,
        "stop": ["Observation:"]
    });
    let resp = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", openai_key))
        .json(&body)
        .send()
        .await?;
    let choices = resp.json::<Value>().await?;
    choices["choices"][0]["text"].as_str().unwrap().to_string()
} 

// Answer a question by constructing prompts for the model 
async fn answer_question(question: &str, tools: &Tools, openai_key: &str) -> String {
    // Construct the prompt with the question and tool descriptions
    let prompt_template = fs::read_to_string("prompt.txt").unwrap();
    let prompt = prompt_template.replace("${question}", question)
        .replace("${tools}", 
            tools.iter().map(|(f, d)| format!("{}: {}", f.to_string().split('.').next().unwrap(), d))
                .collect::<Vec<_>>().join("\n"));
    
    // Iterate until a final answer is found
    let mut prompt = prompt;
    loop {
        let response = complete_prompt(prompt.clone(), openai_key).await;
        prompt.push_str(&response);
        
        if let Some(action) = response.split_once("Action: ") {
            // Execute the specified action
            let action_input = response.split_once("Action Input: \"").unwrap().1.rsplit('\"').nth(1).unwrap();
            let result = (tools.search.0)(action_input.to_string());
            prompt.push_str(&format!("Observation: {}\n", result.unwrap()));
        } else if response.contains("Final Answer: ") {
            return response.split_once("Final Answer: ").unwrap().1.to_string();
        }
    }
}
  
// Merge the chat history with a new question
async fn merge_history(question: &str, history: &str, openai_key: &str) -> String {
    let prompt_template = fs::read_to_string("merge.txt").unwrap();
    let prompt = prompt_template.replace("${question}", question)
        .replace("${history}", history);
    complete_prompt(prompt, openai_key).await
}

#[tokio::main]
async fn main() {
    let openai_key = env::var("OPENAI_API_KEY")?;
    let tools = Tools {
        search: (google_search, "a search engine. useful for when you need to answer questions about current events. input should be a search query."),
        calculator: (|input| eval::eval(&input).to_string(), "Useful for getting the result of a math expression. The input to this tool should be a valid mathematical expression that could be executed by a simple calculator.")
    };
    
    let mut history = String::new();
    let mut rl = rustyline::Editor::<()>::new();
    loop {
        let question = rl.readline("How can I help? ");
        
        if !history.is_empty() {
            question = merge_history(&question.unwrap(), &history, &openai_key).await;
        } 
        
        let answer = answer_question(&question.unwrap(), &tools, &openai_key).await;
        println!("{}", answer);
        history.push_str(&format!("Q:{}\nA:{}\n", question.unwrap(), answer));
    }
}
