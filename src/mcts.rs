use crate::guess::{FeedBack, Guess};
use rand::{seq::SliceRandom, thread_rng};
use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, Write},
    path::Path,
    process,
    sync::Arc,
    thread,
};

const EXPLORATION_FACTOR: f32 = std::f32::consts::SQRT_2;

type StateSpace = HashMap<(Guess, String), Node>;

#[derive(Clone)]
struct Node {
    pub guess: Guess,
    pub cumulative_score: f32,
    pub num_simulations: u32,
    pub num_guess: u8,
}

impl Node {
    pub fn score(&self, parent_num_simulation: u32) -> f32 {
        if self.num_simulations == 0 {
            return 1000.0;
        }
        self.cumulative_score / self.num_simulations as f32
            + EXPLORATION_FACTOR
                * f32::sqrt((parent_num_simulation as f32).ln() / self.num_simulations as f32)
    }
}

pub fn search(
    guess: Guess,
    num_guess: u8,
    max_guess: u8,
    dict: &[String],
    num_iterations: usize,
    cache: &HashMap<(String, String), Vec<FeedBack>>,
    state_space_path: &Path,
    num_threads: usize,
) -> String {
    let arc_dict = Box::new(Arc::new(dict.to_vec()));
    let arc_cache = Box::new(Arc::new(cache.clone()));
    let arc_state_space_path = Box::new(Arc::new(state_space_path.to_path_buf()));
    let mut solutions = guess.solutions(dict);
    solutions.shuffle(&mut thread_rng());
    let state_space: StateSpace = solutions
        .chunks(num_threads)
        .map(|x| x.to_vec())
        .into_iter()
        .fold(StateSpace::new(), |all_solutions_state_space, solutions| {
            let handles: Vec<thread::JoinHandle<StateSpace>> = solutions
                .into_iter()
                .map(|solution| {
                    println!("{}", solution);
                    let local_solution = solution.to_string();
                    let local_guess = guess.clone();
                    let local_dict = (*arc_dict).clone();
                    let local_cache = (*arc_cache).clone();
                    let local_state_space_path = (*arc_state_space_path).clone();
                    let mut state = StateSpace::new();
                    if local_state_space_path
                        .join(&format!("{}.zst", solution))
                        .exists()
                    {
                        if let Ok(()) =
                            uncompress_state_space(&local_state_space_path, &local_solution)
                        {
                            if let Ok(s) = load_state_space_from_file(
                                &local_state_space_path
                                    .join(&Path::new(&format!("{}.csv", local_solution))),
                            ) {
                                state = s;
                            }
                        }
                    }
                    std::thread::spawn(move || {
                        let state = explore_one_solution(
                            state,
                            local_guess,
                            local_solution.clone(),
                            num_guess,
                            max_guess,
                            &local_dict,
                            num_iterations,
                            &local_cache,
                        );
                        save_state_space_to_file(
                            &state,
                            &local_state_space_path
                                .join(Path::new(&format!("{}.csv", local_solution))),
                        )
                        .unwrap();
                        compress_state_space(&local_state_space_path, &local_solution).unwrap();
                        state
                    })
                })
                .collect();
            let mut state_spaces: Vec<StateSpace> = handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();
            state_spaces.push(all_solutions_state_space);
            combine_state_spaces(state_spaces)
        });
    // Once the MCTS is done, we can select the best guess
    select_best_word(&guess, &state_space, dict)
}

fn save_state_space_to_file(state_space: &StateSpace, filename: &Path) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(filename)?;
    for ((guess, word), node) in state_space {
        writeln!(
            &mut file,
            "{},{},{},{},{},{}",
            guess.to_string(),
            word,
            node.guess.to_string(),
            node.cumulative_score,
            node.num_simulations,
            node.num_guess,
        )
        .unwrap();
    }
    file.sync_all()?;
    Ok(())
}

fn compress_state_space(state_space: &Path, solution: &str) -> io::Result<()> {
    process::Command::new("tar")
        .args([
            "-c",
            "-I",
            "zstd",
            "-f",
            &state_space
                .join(&format!("{}.zst", solution))
                .to_str()
                .unwrap(),
            "-C",
            &state_space.to_str().unwrap(),
            &format!("{}.csv", solution),
        ])
        .spawn()?
        .wait()?;
    fs::remove_file(&state_space.join(&format!("{}.csv", solution)))?;
    Ok(())
}

fn uncompress_state_space(state_space: &Path, solution: &str) -> io::Result<()> {
    process::Command::new("tar")
        .args([
            "-x",
            "-I",
            "zstd",
            "-f",
            &state_space
                .join(&format!("{}.zst", solution))
                .to_str()
                .unwrap(),
            "-C",
            &state_space.to_str().unwrap(),
        ])
        .spawn()?
        .wait()?;
    Ok(())
}

fn load_state_space_from_file(filename: &Path) -> io::Result<StateSpace> {
    let file = fs::File::open(filename)?;
    let mut state_space = StateSpace::new();
    for line in io::BufReader::new(file).lines() {
        let line = line?;
        let line: Vec<&str> = line.split(',').collect();
        if line.len() < 6 {
            continue;
        }
        let guess = Guess::from_string(line[0]);
        let word = line[1].to_string();
        let node_guess = Guess::from_string(line[2]);
        let cumulative_score = line[3].parse::<f32>();
        let num_simulations = line[4].parse::<u32>();
        let num_guess = line[5].parse::<u8>();
        if cumulative_score.is_err() || num_simulations.is_err() || num_guess.is_err() {
            continue;
        }
        let cumulative_score = cumulative_score.unwrap();
        let num_simulations = num_simulations.unwrap();
        let num_guess = num_guess.unwrap();
        state_space.insert(
            (guess, word.to_string()),
            Node {
                guess: node_guess,
                cumulative_score,
                num_simulations,
                num_guess,
            },
        );
    }
    Ok(state_space)
}

fn combine_state_spaces(state_spaces: Vec<StateSpace>) -> StateSpace {
    let mut combined_state_space = StateSpace::new();
    for state_space in state_spaces {
        for (key, new_node) in state_space {
            if let Some(node) = combined_state_space.get_mut(&key) {
                node.cumulative_score += new_node.cumulative_score;
                node.num_simulations += new_node.num_simulations;
            } else {
                combined_state_space.insert(key, new_node);
            }
        }
    }
    combined_state_space
}

fn select_best_word(initial_guess: &Guess, state_space: &StateSpace, dict: &[String]) -> String {
    initial_guess
        .solutions(dict)
        .into_iter()
        .map(|word| {
            let state = state_space
                .get(&(initial_guess.clone(), word.clone()))
                .unwrap();
            (word, state.cumulative_score / state.num_simulations as f32)
        })
        .fold(
            (String::new(), -1000.0),
            |(best_word, best_score), (word, score)| {
                if score > best_score {
                    (word, score)
                } else {
                    (best_word, best_score)
                }
            },
        )
        .0
}

fn explore_one_solution(
    mut state_space: StateSpace,
    guess: Guess,
    solution: String,
    num_guess: u8,
    max_guess: u8,
    dict: &[String],
    num_iterations: usize,
    cache: &HashMap<(String, String), Vec<FeedBack>>,
) -> StateSpace {
    let dict = &dict[..];
    let root = Node {
        guess: guess.clone(),
        cumulative_score: 0.0,
        num_simulations: 0,
        num_guess,
    };
    state_space.insert((guess.clone(), "".to_string()), root.clone());
    // Insert initial children
    for word in guess.solutions(dict) {
        let feedback = FeedBack::evaluate(&word, &solution, cache);
        state_space.insert(
            (guess.clone(), word.clone()),
            Node {
                guess: guess.refine(&word, &feedback, dict),
                cumulative_score: 0.0,
                num_simulations: 0,
                num_guess: num_guess + 1,
            },
        );
    }
    for _ in 0..num_iterations {
        //println!("{} {}/{}", solution, i + 1, num_iterations);
        // One iteration of MCTS
        let mut sequence = vec![(root.guess.clone(), "".to_string())];
        let mut current_node = &root;
        // Selection
        loop {
            let children_words = current_node.guess.solutions(dict);
            // Select until a leaf node, an unexplored node or a terminal node is found
            if children_words.len() <= 1
                || current_node.num_simulations == 0
                || children_words.iter().any(|word| {
                    !state_space.contains_key(&(current_node.guess.clone(), word.to_string()))
                })
            {
                break;
            }
            let ((selected_node, selected_word), _) = children_words
                .iter()
                .map(|word| {
                    let node = state_space
                        .get(&(current_node.guess.clone(), word.to_string()))
                        .unwrap();
                    ((node, word), node.score(current_node.num_simulations))
                })
                .fold(
                    ((current_node, ""), -1.0),
                    |(acc, max_score), ((node, w), score)| {
                        if score > max_score {
                            ((&node, w), score)
                        } else {
                            (acc, max_score)
                        }
                    },
                );
            sequence.push((current_node.guess.clone(), selected_word.to_string()));
            current_node = selected_node;
        }
        let leaf_guess = current_node.guess.clone();
        let leaf_num_guess = current_node.num_guess;
        drop(current_node);
        if leaf_num_guess < max_guess && leaf_guess.num_solutions() > 1 {
            // Expand the leaf node
            for word in leaf_guess.solutions(dict) {
                let feedback = FeedBack::evaluate(&word, &solution, cache);
                let new_guess = leaf_guess.refine(&word, &feedback, dict);
                state_space.insert(
                    (leaf_guess.clone(), word.clone()),
                    Node {
                        guess: new_guess,
                        cumulative_score: 0.0,
                        num_simulations: 0,
                        num_guess: num_guess + 1,
                    },
                );
            }
            // Randomly select a child node
            let child_word = leaf_guess
                .solutions(dict)
                .choose(&mut thread_rng())
                .unwrap()
                .to_string();
            sequence.push((leaf_guess.clone(), child_word.clone()));
            // Simulation
            let mut simulation_node = state_space
                .get(&(leaf_guess.clone(), child_word))
                .unwrap()
                .clone();
            while simulation_node.num_guess < max_guess && simulation_node.guess.num_solutions() > 1
            {
                let children_words = simulation_node.guess.solutions(dict);
                let child_word = children_words
                    .choose(&mut thread_rng())
                    .unwrap()
                    .to_string();

                let next_guess = simulation_node.guess.refine(
                    &child_word,
                    &FeedBack::evaluate(&child_word, &solution, cache),
                    dict,
                );
                simulation_node = Node {
                    guess: next_guess,
                    cumulative_score: 0.0,
                    num_simulations: 0,
                    num_guess: simulation_node.num_guess + 1,
                };
            }
            if simulation_node.guess.num_solutions() == 1 {
                // Simulation won
                simulation_node.cumulative_score =
                    max_guess as f32 - simulation_node.num_guess as f32;
            } else {
                // Simulation lost
                simulation_node.cumulative_score = 0.0;
            }
            // Backpropagation
            // Add the cumulative score and increment num_simulation up the entire chain
            for (guess, word) in sequence.iter().rev() {
                let node = state_space.get_mut(&(guess.clone(), word.clone())).unwrap();
                node.cumulative_score += simulation_node.cumulative_score;
                node.num_simulations += 1;
            }
        } else {
            // Leaf node is terminal, we immediately backpropagate
            let mut cumulative_score = 0.0;
            if leaf_guess.num_solutions() == 1 {
                // Simulation won
                cumulative_score = max_guess as f32 - leaf_num_guess as f32;
            }
            // Backpropagation
            // Add the cumulative score and increment num_simulation up the entire chain
            for (guess, word) in sequence.iter().rev() {
                let node = state_space.get_mut(&(guess.clone(), word.clone())).unwrap();
                node.cumulative_score += cumulative_score;
                node.num_simulations += 1;
            }
        }
    }
    return state_space;
}
