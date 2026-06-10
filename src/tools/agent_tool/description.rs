use crate::agent::profiles::AgentProfile;

const BASE_DESCRIPTION: &str = "\
Launch a new agent to handle complex, multistep tasks autonomously. \
Use the profile parameter to select the agent type, such as explorer, verifier, planner, implementer, or a project profile. \
Use role only as an advanced override when you need a raw role instead of a named profile. \
\
When NOT to use the agent tool: if you want to read a specific file, use file_read or glob; \
if you are searching for a specific class definition, use grep; \
if you are searching within 2-3 files, use file_read instead. \
If no available profile is a good fit, use other tools directly. \
\
Usage notes: \
1. Launch multiple agents concurrently whenever possible, to maximize performance; use a single message with multiple tool uses. \
2. Once you have delegated work to an agent, do not duplicate that work yourself. Continue with non-overlapping tasks or wait for the result. \
3. The result returned by the agent is not visible to the user. To show the user the result, send a text message with a concise summary. \
4. Each agent starts with a fresh context. Provide a highly detailed prompt describing what the agent should do autonomously and what it should return. \
5. Tell the agent whether to write code or only do research (file reads, searches), since it cannot see the user's original request.";

pub(super) fn build_tool_description(profiles: &[AgentProfile]) -> String {
    let mut description = BASE_DESCRIPTION.to_string();
    let subagents = profiles
        .iter()
        .filter(|profile| profile.name != "default" && !profile.name.is_empty());

    let mut wrote_header = false;
    for profile in subagents {
        if !wrote_header {
            description.push_str("\n\nAvailable profiles and their intended use:\n\n");
            wrote_header = true;
        }
        description.push_str(&format!("- {}: {}\n", profile.name, profile.description));
    }
    description
}
