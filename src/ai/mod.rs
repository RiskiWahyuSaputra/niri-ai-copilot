pub mod actions;
pub mod groq;
pub mod prompts;
pub mod provider;
pub mod question;

pub use actions::{
    ask_practice_question, evaluate_practice_session, explain_term, generate_alternative,
    generate_suggestion, render_report_markdown, summarize_session, test_connection,
};
pub use prompts::{PracticeReport, Suggestion};
pub use provider::{build_provider, AiProvider, CompletionRequest, Message};
pub use question::{looks_like_question, QuestionCandidate, QuestionDetector};
