use crate::engine::retrieval_context::RetrievalContext;
use crate::services::api::Message;

pub(super) struct RetrievalPromptContext<'a> {
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct RetrievalPromptController;

impl RetrievalPromptController {
    pub(super) fn inject(context: RetrievalPromptContext<'_>) -> bool {
        let Some(retrieval_context) = context.retrieval_context else {
            return false;
        };
        let block = retrieval_context.format_for_prompt();
        Self::inject_block(context.messages, &block)
    }

    fn inject_block(messages: &mut Vec<Message>, block: &str) -> bool {
        if block.is_empty()
            || messages
                .iter()
                .any(|message| matches!(message, Message::System { content } if content.contains("project.index:")))
        {
            return false;
        }
        messages.push(Message::system(block));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_nonempty_retrieval_block() {
        let mut messages = vec![Message::user("inspect repo")];

        assert!(RetrievalPromptController::inject_block(
            &mut messages,
            "<retrieval-context>\nproject.index: src/main.rs\n</retrieval-context>",
        ));

        assert_eq!(messages.len(), 2);
        assert!(matches!(
            &messages[1],
            Message::System { content } if content.contains("project.index: src/main.rs")
        ));
    }

    #[test]
    fn skips_empty_or_existing_project_index_block() {
        let mut messages = vec![Message::user("inspect repo")];

        assert!(!RetrievalPromptController::inject_block(&mut messages, ""));
        assert_eq!(messages.len(), 1);

        messages.push(Message::system("project.index: existing"));
        assert!(!RetrievalPromptController::inject_block(
            &mut messages,
            "project.index: new",
        ));
        assert_eq!(messages.len(), 2);
    }
}
