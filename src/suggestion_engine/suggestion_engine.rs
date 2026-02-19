use fuzzy_matcher::skim::SkimMatcherv2;
use fuzzy_matcher::FuzzyMatcher;

pub struct Suggestion_Engine{
    pub static_commands: Vec<String>;
    pub history: Vec<String>;
    pub ai_cache: Vec<String>;
}

impl Suggestion_Engine{
    pub fn suggest(&self, input:&str)-> Vec<String>{
        let matcher = SkimMatcherv2.default();
        let mut scored = Vec<i64,String> = Vec::new();
    }
}