use anyhow::{anyhow, Result};
use log::info;
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Seek, Write},
    path::{Path, PathBuf},
};

const TEMP_DIR: &str = "/tmp/uhhyeahhh";

/// assumed list is sorted
fn find_compound_word(word_char_map: HashMap<Vec<char>, &str>) -> HashMap<&str, Vec<&str>> {
    // finding compounds
    let mut compound_word_hashmap = HashMap::new();
    for current_word in word_char_map.values() {
        let current_word_chars = current_word.chars().collect::<Vec<_>>();
        let char_count = current_word_chars.len();
        let mut is_compound = false;
        let mut current_compoenets = vec![];
        let mut start = 0;
        let mut end = char_count - 1;
        loop {
            let current_word_slice = &current_word_chars[start..end];
            let sub_word = word_char_map.get(&current_word_slice.to_vec());
            if let Some(sub_word) = sub_word {
                current_compoenets.push(*sub_word);
                start = end;
                end = char_count;
            } else {
                end = match end.checked_sub(1) {
                    Some(e) => e,
                    None => break,
                };
                if end < start {
                    break;
                }
            }
            if start == char_count {
                is_compound = true;
                break;
            }
        }

        if is_compound {
            compound_word_hashmap.insert(*current_word, current_compoenets);
        }
    }

    fn recursively_split_compound_components<'b>(
        compound_word_hashmap: &HashMap<&'b str, Vec<&'b str>>,
        components: Vec<&'b str>,
    ) -> Vec<&'b str> {
        components
            .into_iter()
            .flat_map(|word| match compound_word_hashmap.get(word) {
                Some(compound) => {
                    recursively_split_compound_components(compound_word_hashmap, compound.clone())
                }
                None => vec![word],
            })
            .collect()
    }

    let compound_words = compound_word_hashmap.keys().copied().collect::<Vec<_>>();
    for compound_word in compound_words {
        let components = compound_word_hashmap.remove(compound_word).unwrap();
        let components = recursively_split_compound_components(&compound_word_hashmap, components);
        compound_word_hashmap.insert(compound_word, components);
    }

    compound_word_hashmap
}

// fn compound_centipede<'a>(
//     compound_words: HashMap<&str, Vec<&'a str>>,
//     centipede_len: usize,
//     min_meaning_length: usize,
// ) -> Vec<&'a str> {
//     let meaining_length = centipede_len.min(min_meaning_length);

// }

// struct CompoundWordTree<'a> {
//     nodes: CompoundWordTreeNode<'a>,
// }

// impl<'a> CompoundWordTree<'a> {
//     // fn new(, compound_words: HashMap<&str, Vec<&'a str>>) -> CompoundWordTree<'a> {
//     //     path
//     // }
// }
#[derive(Debug)]
struct SuccessorCache<'a>(HashMap<&'a str, HashSet<&'a str>>);

impl<'a> SuccessorCache<'a> {
    fn new(words: &OrganizedWords<'a>) -> SuccessorCache<'a> {
        let mut map = HashMap::new();
        for word in &words.non_compound_words {
            let successor = CompoundWordTree::find_successors(words, word);
            map.insert(*word, successor);
        }
        SuccessorCache(map)
    }

    fn successor_for(&self, word: &'a str) -> Option<&HashSet<&'a str>> {
        self.0.get(word)
    }
}

#[derive(Debug)]
struct CompoundWordTree<'a> {
    word: &'a str,
    nexts: Vec<CompoundWordTree<'a>>,
}

impl<'a> CompoundWordTree<'a> {
    #[allow(unused)]
    fn new(
        words: &OrganizedWords<'a>,
        used_word: &mut HashSet<&'a str>,
        word: &'a str,
        limit: u64,
    ) -> CompoundWordTree<'a> {
        if limit == 0 {
            return CompoundWordTree {
                word,
                nexts: vec![],
            };
        };
        let nexts = CompoundWordTree::find_successors(words, word);
        let nexts = nexts.difference(used_word).copied().collect::<Vec<_>>();
        used_word.extend(&nexts);
        CompoundWordTree {
            word,
            nexts: nexts
                .into_iter()
                .map(|next| CompoundWordTree::new(words, used_word, next, limit - 1))
                .collect(),
        }
    }

    fn new_from_cache(
        successor_cache: &SuccessorCache<'a>,
        used_word: &mut HashSet<&'a str>,
        word: &'a str,
        limit: u64,
    ) -> CompoundWordTree<'a> {
        if limit == 0 {
            return CompoundWordTree {
                word,
                nexts: vec![],
            };
        };
        let nexts = successor_cache
            .successor_for(word)
            .cloned()
            .unwrap_or_default();
        let nexts = nexts.difference(used_word).copied().collect::<Vec<_>>();
        used_word.extend(&nexts);
        CompoundWordTree {
            word,
            nexts: nexts
                .into_iter()
                .map(|next| {
                    CompoundWordTree::new_from_cache(successor_cache, used_word, next, limit - 1)
                })
                .collect(),
        }
    }

    fn find_successors(words: &OrganizedWords<'a>, word: &'a str) -> HashSet<&'a str> {
        let mut sucessors = HashSet::new();
        for other_word in words.non_compound_words.iter() {
            let test_word = word.to_string() + other_word;
            if words.compound_words.contains_key(&test_word[..]) {
                sucessors.insert(*other_word);
            }
        }
        sucessors
    }

    #[allow(unused)]
    fn count(&self) -> usize {
        1 + self.nexts.iter().map(|next| next.count()).sum::<usize>()
    }

    fn graph(&self) -> Vec<Vec<&'a str>> {
        let mut lines = vec![vec![self.word]];
        for next_node in &self.nexts {
            let graphed_next_node_lines = next_node.graph();
            for graphed_next_node_line in graphed_next_node_lines {
                let mut line = vec![self.word];
                line.extend(graphed_next_node_line);
                lines.push(line);
            }
        }
        lines
    }
}

struct OrganizedWords<'a> {
    compound_words: HashMap<&'a str, Vec<&'a str>>,
    non_compound_words: HashSet<&'a str>,
}

fn main() -> Result<()> {
    env_logger::init();
    let all_words = fs::read_to_string("all_words")?;
    let all_words = all_words.lines().collect::<Vec<_>>();
    let mut all_words = all_words
        .iter()
        .filter(|word| {
            !word.contains(' ')
                && word.chars().count() > 1
                && !word.contains('ๆ')
                && !word.contains('-')
                && !word.contains("กระ")
        })
        .copied()
        .collect::<Vec<_>>();

    all_words.sort_by_key(|word| word.chars().count());
    all_words.reverse();
    let word_char_map = HashMap::from_iter(
        all_words
            .iter()
            .map(|word| (word.chars().collect::<Vec<_>>(), *word)),
    );
    let compound_words = find_compound_word(word_char_map);
    let non_compound_words = compound_words
        .values()
        .flatten()
        .copied()
        .collect::<HashSet<_>>();
    let words = OrganizedWords {
        compound_words,
        non_compound_words,
    };

    all_possible_compound_centipede(&words, u64::MAX, "all.txt")?;

    Ok(())
}

fn possible_compound_centipede_with_start<'a>(
    successor_cache: &SuccessorCache<'a>,
    word: &'a str,
    max_len: u64,
) -> Vec<Vec<&'a str>> {
    let mut used_word = HashSet::new();
    let tree = CompoundWordTree::new_from_cache(successor_cache, &mut used_word, word, max_len);
    tree.graph()
}

fn all_possible_compound_centipede<P: AsRef<Path>>(
    words: &OrganizedWords<'_>,
    max_len: u64,
    output: P,
) -> Result<()> {
    match fs::create_dir_all(TEMP_DIR) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(anyhow!(e));
        }
    };
    let path = PathBuf::from(TEMP_DIR).canonicalize().unwrap();
    info!("Temp directory is \"{}\"", path.display());
    info!("Calculating and caching successors.");
    let successor_cache = SuccessorCache::new(words);
    let mut output_file = fs::File::options().create(true).append(true).open(output)?;
    words
        .non_compound_words
        .par_iter()
        .map(|word| {
            info!("calculating for \"{word}\".");
            let word_graph =
                possible_compound_centipede_with_start(&successor_cache, word, max_len);
            let temp_dir = TEMP_DIR;
            let mut this_word_file = fs::File::options()
                .write(true)
                .truncate(true)
                .create(true)
                .open(format!("{temp_dir}/{word}"))?;
            for line in word_graph {
                for word in line {
                    write!(this_word_file, "{word} ")?;
                }
                writeln!(this_word_file)?;
            }
            Result::<(), anyhow::Error>::Ok(())
        })
        .collect::<Vec<Result<(), anyhow::Error>>>();
    info!("merging files.");
    output_file.set_len(0)?;
    output_file.seek(io::SeekFrom::End(0))?;
    for files in fs::read_dir(TEMP_DIR)? {
        let file = files?;
        println!("{}", file.path().display());
        let mut file = fs::File::options().read(true).open(file.path())?;
        io::copy(&mut file, &mut output_file)?;
    }
    info!("finished.");
    // fs::remove_dir_all(TEMP_DIR)?;
    Ok(())
}
