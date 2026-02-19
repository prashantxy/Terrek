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
        let sources = self 
        .static_commands
        .iter()
        .chain(self.history.iter())
        .chain(self.ai_cache.iter())

        for item in sources {
            if let Some(score) = matcher.fuzzy_match(item,input){
                scored.push((score,item.clone()));
            }
        }
        scored.sort_by(| a,b |,b.0.cmp(&a.0));
        scored.into_iter().map(|(_,s)|s).take(5).collect()
    }
}