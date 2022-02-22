use std::collections::HashMap;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Guess {
    possible_words: Vec<bool>,
}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub enum FeedBack {
    Black,
    Yellow,
    Green,
}

impl FeedBack {
    pub fn evaluate(
        word: &str,
        solution: &str,
        cache: &HashMap<(String, String), Vec<FeedBack>>,
    ) -> Vec<FeedBack> {
        if let Some(feedback) = cache.get(&(word.to_string(), solution.to_string())) {
            return feedback.clone();
        }
        let feedback: Vec<FeedBack> = word
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if solution.contains(c) {
                    if solution.chars().nth(i) == Some(c) {
                        FeedBack::Green
                    } else {
                        FeedBack::Yellow
                    }
                } else {
                    FeedBack::Black
                }
            })
            .collect();
        feedback
    }
}

impl Guess {
    pub fn new(dict: &[String]) -> Self {
        let possible_words = vec![true; dict.len()];
        Self { possible_words }
    }
    pub fn from_string(possible_words: &str) -> Self {
        let possible_words = possible_words
            .chars()
            .map(|c| c == '1')
            .collect::<Vec<bool>>();
        Self { possible_words }
    }
    pub fn to_string(&self) -> String {
        self.possible_words
            .iter()
            .map(|&b| if b { "1".to_string() } else { "0".to_string() })
            .collect::<Vec<String>>()
            .join("")
    }
    pub fn solutions(&self, dict: &[String]) -> Vec<String> {
        self.possible_words
            .iter()
            .enumerate()
            .filter(|(_, &b)| b)
            .map(|(i, _)| dict[i].clone())
            .collect()
    }
    pub fn num_solutions(&self) -> usize {
        self.possible_words.iter().filter(|&&b| b).count()
    }
    pub fn refine(&self, word: &str, feedback: &[FeedBack], dict: &[String]) -> Self {
        if feedback.iter().all(|fb| *fb == FeedBack::Green) {
            let mut possible_words = vec![false; dict.len()];
            possible_words[dict
                .iter()
                .position(|w| w == word)
                .expect("word not found in dict")] = true;
            return Self { possible_words };
        }
        let new_possible_words = self
            .possible_words
            .iter()
            .enumerate()
            .map(|(i, &possible)| {
                let w = &dict[i];
                possible
                    && feedback
                        .iter()
                        .enumerate()
                        .fold(true, |acc, (j, &feedback)| {
                            acc && match feedback {
                                FeedBack::Black => w.chars().nth(j) != word.chars().nth(j),
                                FeedBack::Green => w.chars().nth(j) == word.chars().nth(j),
                                FeedBack::Yellow => w
                                    .chars()
                                    .enumerate()
                                    .any(|(k, c)| k != j && Some(c) == word.chars().nth(j)),
                            }
                        })
            })
            .collect();
        Self {
            possible_words: new_possible_words,
        }
    }
}
