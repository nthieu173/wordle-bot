mod guess;
mod mcts;
mod word;

use clap::Parser;
use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(
        short,
        long,
        default_value = "./official.txt",//"/usr/share/dict/words",
        help = "Path to word list"
    )]
    dict: PathBuf,
    #[clap(short, long, default_value = "./cache.txt", help = "Solution cache")]
    cache: PathBuf,
    #[clap(
        short,
        long,
        default_value = "./state",
        help = "Path to state space folder"
    )]
    state_space: PathBuf,
    #[clap(
        short,
        long,
        default_value_t = 100,
        help = "Number of iterations per word"
    )]
    iterations: usize,
    #[clap(short, long, default_value_t = 4, help = "Number of threads used")]
    thread: usize,
    #[clap(short, long, default_value_t = 5, help = "Word length")]
    length: u8,
    #[clap(short, long, default_value_t = 6, help = "Max number of guesses")]
    max_guess: u8,
    #[clap(help = "Guesses so far")]
    guesses: Vec<String>,
}

pub fn generate_cache(dict: &[String]) -> HashMap<(String, String), Vec<guess::FeedBack>> {
    let mut cache = HashMap::new();
    for word in dict {
        for solution in dict {
            guess::FeedBack::evaluate(word, solution, &mut cache);
        }
    }
    cache
}

pub fn export_cache(cache: &HashMap<(String, String), Vec<guess::FeedBack>>, out: &Path) {
    let mut file = fs::File::create(out).unwrap();
    for (key, value) in cache {
        let mut line = "".to_string() + &key.0 + "," + &key.1 + ",";
        for fb in value {
            line += match fb {
                guess::FeedBack::Green => "g",
                guess::FeedBack::Yellow => "y",
                guess::FeedBack::Black => "b",
            };
        }
        line += "\n";
        file.write_all(line.as_bytes()).unwrap();
    }
}

pub fn import_cache(
    inp: &Path,
) -> Result<HashMap<(String, String), Vec<guess::FeedBack>>, io::Error> {
    let file = BufReader::new(fs::File::open(inp)?);
    let mut cache = HashMap::new();
    for line in file.lines() {
        let line = line?;
        let mut iter = line.split(',');
        let word = iter.next().unwrap();
        let solution = iter.next().unwrap();
        let mut feedback = Vec::new();
        for fb in iter.next().unwrap().chars() {
            match fb {
                'g' => feedback.push(guess::FeedBack::Green),
                'y' => feedback.push(guess::FeedBack::Yellow),
                'b' => feedback.push(guess::FeedBack::Black),
                _ => panic!("Invalid feedback"),
            }
        }
        cache.insert((word.to_string(), solution.to_string()), feedback);
    }
    Ok(cache)
}

fn main() {
    let args = Args::parse();
    let dict: Vec<String> = fs::read_to_string(&args.dict)
        .expect("Failed to read dictionary")
        .lines()
        .filter(|word| word.len() == args.length as usize && word::is_clean(word))
        .map(|word| word.to_string())
        .collect();
    let mut cache;
    if let Ok(c) = import_cache(&args.cache) {
        cache = c;
    } else {
        cache = generate_cache(&dict);
        export_cache(&cache, &args.cache);
    }
    let guess = guess::Guess::new(&dict);
    println!(
        "{}",
        mcts::search(
            guess,
            0,
            args.max_guess,
            &dict,
            args.iterations,
            &mut cache,
            &args.state_space,
            args.thread
        )
    );
    //let mut guess = guess::Guess::new(args.length);
    //println!("Hello {:?}!", args.dict)
}
