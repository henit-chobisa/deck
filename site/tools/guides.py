"""The guides: one page for each question people ask when an agent writes the
code and the decisions start slipping past them.

Each guide answers its question honestly first, and shows where deck helps
second. Edit the text here and run

    python3 site/tools/build.py

to write the pages, the hub, the sitemap and robots.txt.
"""

GUIDES = [
    {
        "slug": "ai-code-review",
        "title": "AI code review: how to review code your agent wrote",
        "h1": "How to review code your agent wrote",
        "description": "Reviewing AI-generated code is not like reviewing a colleague's. Here is what to look for, why diffs and summaries fall short, and how to see the decisions your agent made.",
        "eyebrow": "AI code review",
        "body": """
Your agent finished. The diff is 900 lines across eleven files, and the summary says it "refactored the pricing flow and added caching". You have to approve it, and you did not write any of it.

Reviewing code an agent wrote is a different job from reviewing a colleague's. A colleague made a few decisions and can tell you why. An agent made dozens, quickly, and the reasons live in a conversation you may have skimmed.

## Review the decisions, not the lines

You do not need to read every line. You need to find the handful of places where something was **decided**: a timeout chosen, a cache added, an error swallowed, a query moved, a field made optional. Those are the lines that will page someone at three in the morning.

A useful order:

1. **What was the goal?** Write it in one sentence before you open the diff.
2. **Where did behaviour change?** Not where text changed. Formatting, renames and moved files are noise.
3. **What did it assume?** About load, about data shape, about who calls this.
4. **What did it not do?** Tests it did not write, cases it did not handle, old paths it did not remove.

## Why the diff and the summary both fall short

A diff shows every changed line with the same weight. The decision on line 412 looks exactly like the import on line 3.

A summary has the opposite problem. It is the agent's own account of what it did, in prose, with `file:line` references you have to open one at a time and hold in your head. By the fourth file you are assembling the argument yourself.

## Make the agent show you

The fix is to ask for the review in a different shape: one claim at a time, each with the code that proves it, in front of you.

That is what [deck](/) does. Your agent writes a short deck instead of a paragraph. Each group is one claim, and the lines it rests on are open beside it. Click a sentence and its lines light up. When the claim is about behaviour over time (a queue backing up, two requests racing), the agent can draw it as a small chart that moves with the sentence.

When you disagree, you select the lines and say so. Your comment goes back to the same agent, pinned to those lines, and it picks up from there.

## A checklist for agent-written changes

- Every new dependency, cache, retry or timeout: why this value?
- Every catch block: what is swallowed, and who would notice?
- Every query or loop that touches the network: what happens at ten times the load?
- Every deleted line: was anything else relying on it?
- Every test: does it test the behaviour, or the implementation the agent just wrote?

Ask your agent to answer those as a deck, and you review the reasoning instead of hunting for it.
""",
        "faq": [
            ("Do I still need to read the diff?", "Yes, for the parts that matter. The point is to find them quickly: the decisions, not the formatting. A deck puts those first and leaves the rest in the diff."),
            ("Can an AI agent review its own code?", "It can explain and defend it, which is useful. The judgement is still yours. deck is built so the agent makes its case on the lines and you decide."),
            ("Does deck replace pull requests or code review tools?", "No. It sits before the approval: your agent shows you what it did and why, you push back, and then the change goes through your normal review."),
        ],
    },
    {
        "slug": "understand-ai-generated-code",
        "title": "How to understand AI-generated code you didn't write",
        "h1": "Understanding code your agent wrote",
        "description": "When an AI agent writes most of the code, understanding it becomes the hard part. A practical way to rebuild your mental model of what changed and why.",
        "eyebrow": "Understanding AI code",
        "body": """
The strange part of working with a coding agent is not that it writes code. It is that a week later you own a system you cannot fully explain.

Nothing is wrong, exactly. The tests pass. But somebody asks why checkout calls the cache twice, and you realise you approved that on a Tuesday without ever deciding it.

## Why understanding slips

Agents work faster than anyone can follow. Each change is reasonable on its own. The understanding you used to build by writing the code yourself never gets built, because you did not write it.

So understanding has to be rebuilt on purpose, and it is cheapest right after the change, while the agent still holds the whole context.

## Ask for the model, not the summary

A good explanation of a change has three parts:

1. **The situation.** What a person was doing when this code runs. "A customer presses Pay" is better than "the `placeOrder` method".
2. **The mechanism.** The few lines that carry the behaviour, in order.
3. **The decision.** What was chosen, what the alternative was, and what it costs.

Most agents give you the middle part in prose, with file names you have to open. That is the part your eyes can check, and it is the part you are least able to check from a paragraph.

## See it instead of reading about it

[deck](/) turns that explanation into something you look at. Your agent writes it as a deck. The narration sits on top, the code sits underneath, and each sentence lights the lines it is about. When something moves over time, like requests piling up behind a pool or two writes racing, it gets a small chart that plays in step with the words.

You click through it at your own pace. Where it does not add up, you select the lines and ask. The answer comes back in the same window, from the same agent, sometimes as a new chart or another file brought in beside the first.

## A habit that keeps understanding

- After any change you would struggle to explain, ask your agent to show you it before you move on.
- Keep each explanation to one concern. Three short decks beat one long one.
- End each one on a decision you actually make, so the choice is yours on record.
""",
        "faq": [
            ("How do I get my coding agent to explain its changes clearly?", "Ask for one claim at a time with the code beside it, starting from what a user was doing. deck gives your agent a format for exactly that, and checks that every line it points at exists."),
            ("Is it bad to merge code I don't fully understand?", "It is how most incidents start. You do not need to understand every line, but you should understand every decision."),
            ("Does deck work offline?", "Yes. It runs on your machine and opens your files there. Only the optional voice sends the narration, not your code, to Google to be spoken."),
        ],
    },
    {
        "slug": "review-large-pull-requests",
        "title": "How to review a large pull request (even 20,000 lines)",
        "h1": "How to review a 20,000-line pull request",
        "description": "Huge PRs get rubber-stamped. A practical approach to reviewing very large pull requests by concern, in chapters, with the decisions shown on the lines.",
        "eyebrow": "Large pull requests",
        "body": """
Nobody reviews a 20,000-line pull request. They scroll, check that CI is green, leave a comment on something small, and approve. Then the incident review finds the decision that mattered on page forty.

## Split by concern, not by file

A large branch is usually several changes stacked together: a new data model, the code that writes it, the UI that shows it, a migration. Reviewing it file by file mixes all of them on every screen.

Review it in **chapters**, one concern each:

1. Agree the chapters first. Usually three to five.
2. Review chapter one completely before looking at chapter two.
3. Let what you learned in one chapter shape the next.

## Inside each chapter, find the decisions

In each chapter there are only a few places where something was chosen. Those deserve your full attention. The rest is consequence, and a skim is enough for it.

Ask the author, or the agent, for:

- the entry point, from the user's side
- the two or three lines where behaviour actually changed
- what the tests prove, and what they do not

## How deck handles a big review

[deck](/) was shaped by exactly this case. Ask your agent to review the branch with deck, and it proposes the chapters first and asks before it starts. Each chapter opens as its own deck: a map of the branch, then the changes one claim at a time, with the code lit beside each sentence. A change shows as the old lines going and the new lines arriving.

When something looks wrong you comment on the line. Your review of chapter one goes back to the agent before it writes chapter two, so the review gets sharper as it goes instead of more tired.

## If you are the author

Make the reviewer's job possible. Split the branch, or at least say what the chapters are. Put the risky decisions first. Say plainly what you could not test. A reviewer who trusts your map will read the parts that matter.
""",
        "faq": [
            ("How long should a pull request be?", "Short enough that one concern fits in one sitting. When that is not possible, review it in chapters, one concern at a time."),
            ("Can an AI agent help review a large PR?", "Yes, if it shows you the decisions rather than summarising them. deck lets your agent propose chapters and walk each one on the actual lines."),
            ("Does deck post comments to GitHub?", "No. Your comments go back to your agent, pinned to the lines. What it does next, including updating the PR, is up to you and it."),
        ],
    },
    {
        "slug": "review-claude-code-changes",
        "title": "How to review what Claude Code changed",
        "h1": "Reviewing what Claude Code changed",
        "description": "Claude Code can change dozens of files in one session. How to review its work properly: see each decision on the lines, push back where you disagree, and keep the judgement yours.",
        "eyebrow": "Claude Code",
        "body": """
Claude Code is fast. One session can touch thirty files, and the summary at the end names a dozen of them with line numbers. Reviewing that properly is the hard part.

## The problem with the end-of-session summary

The summary is accurate, and it is still hard to use. Each `file:line` is something you have to open, and the argument only exists once you have opened all of them and held them together in your head. Most people stop after the second file and type "looks good".

## Ask for the review as a deck

[deck](/) gives Claude Code another way to answer. Instead of a paragraph, it writes a short deck: one claim per group, with the code that proves it. A bar appears at the bottom of your screen, and you open it when you are ready.

Inside:

- each sentence lights the lines it is about
- a proposed change shows as the old lines going and the new lines arriving
- behaviour over time, like a queue backing up, plays as a chart in step with the words
- you select lines and comment, or ask, and the same session answers

When you submit, the command Claude Code ran ends and hands your review back, pinned to the lines. It continues with your decision in hand, in the same session with the same context.

## Setting it up

Run the installer, then `deck setup`. Setup installs a short skill so Claude Code knows when to reach for a deck, and it offers **the catch**: a hook that notices when a reply names two or more `file:line` locations and asks for a deck instead. After a long session is compacted, the same hook reminds it to load the skill again, so decks late in a session are as good as early ones.

## What to ask for

- "Show me what you changed, one decision at a time."
- "Before you write it, show me the plan."
- "Why does this break only under load? Show me."

The answer comes back as something you can check, not something you have to believe.
""",
        "faq": [
            ("Does deck run its own model?", "No. Claude Code writes the deck with a few shell commands and puts it on your screen. deck draws it and hands your answer back to the same session."),
            ("Is the catch required?", "No. It is offered during setup and can be turned off. Without it, Claude Code still uses deck when the skill suggests it."),
            ("Does it work with other agents too?", "Yes. Anything that can run a shell command can write a deck, including Codex, Cursor and Amp."),
        ],
    },
    {
        "slug": "review-codex-cursor-changes",
        "title": "Reviewing changes from Codex, Cursor and Amp",
        "h1": "Reviewing what Codex, Cursor or Amp changed",
        "description": "Whichever coding agent you use, the review problem is the same: fast changes, long summaries, decisions you never made. How to see them on the lines instead.",
        "eyebrow": "Codex, Cursor, Amp",
        "body": """
Every coding agent has the same shape of problem at the end of a task. The work is done, it is mostly right, and the explanation is a block of prose with file names in it.

## The agent is not the problem

Codex, Cursor and Amp are all good at writing code. The gap is on the other side: you need to understand what was decided before you ship it, and prose is a poor way to carry that.

## One format for any agent

[deck](/) works with any agent that can run a shell command. The agent writes a deck with four commands: start it, open it, add a group per claim, seal it. Each group says one thing and points at the lines that show it. deck checks that every file and line it points at exists before it accepts the group.

On your side, a bar appears. You open the deck when you are ready, click through the claims, see the code lit beside each one, and comment where you disagree. Your review goes back to the same agent, which carries on from there.

## What changes in practice

- **Reviews get shorter.** You look at the decisions, not the formatting.
- **Disagreements get earlier.** You push back on the plan before the code exists.
- **Context survives.** The agent that did the work is the one answering your questions.

## Setup

Install deck and run `deck setup`. It finds the agents on your machine and installs a short skill into each, so they know when a deck is the better answer. See [which agents deck works with](/works-with/).
""",
        "faq": [
            ("Which agents does deck support?", "Any agent that can run a shell command. Setup installs its skill into Claude Code, Codex, Cursor and Amp when it finds them."),
            ("Do I need an API key?", "No. deck uses the agent you already run. A key is only needed for the optional voice."),
            ("Is deck free?", "Yes. Free and open source under Apache-2.0, with no account and no paid tier."),
        ],
    },
    {
        "slug": "stop-rubber-stamping-pull-requests",
        "title": "LGTM fatigue: how to stop rubber-stamping pull requests",
        "h1": "How to stop rubber-stamping pull requests",
        "description": "\"Looks good to me\" has become the review. Why rubber-stamping happens when agents write the code, and a practical way to review decisions instead of approving them.",
        "eyebrow": "LGTM fatigue",
        "body": """
"LGTM" used to mean you looked. Now it often means you trust the tests, the agent, or the person who asked the agent. Everyone knows it, and the approvals keep coming.

## Why it happens

Rubber-stamping is not laziness. It is what people do when reviewing properly costs more than they have:

- the change is too big to hold in your head
- the explanation is prose you have to translate back into code
- the author, human or agent, has moved on to the next thing

So the review becomes a check that nothing looks alarming, and the decisions go through unexamined.

## Lower the cost of a real review

You will not fix this by asking people to try harder. Make a real review cheaper than a fake one:

1. **Smaller units.** One concern per review.
2. **Decisions first.** The author says where the choices are.
3. **Evidence beside the claim.** No "see line 412". Show line 412.
4. **A cheap way to disagree.** A comment on the exact line, answered by whoever holds the context.

## Where deck fits

[deck](/) is that cheaper review, when an agent did the work. The agent shows you its changes as a short deck, one claim at a time with the code lit beside it. Disagreeing is selecting a line and typing. The same agent answers, in the same session, on the same lines.

It is not a gate and it does not replace your PR process. It is the moment before you approve, where you actually decide.

## Signs it is working

- Review comments name a decision, not a style choice.
- People can explain a change a week after merging it.
- "Why did we do it this way?" has an answer someone remembers choosing.
""",
        "faq": [
            ("What does rubber-stamping a pull request mean?", "Approving it without really reviewing it, usually because a proper review would take longer than anyone has."),
            ("How do I get my team to review AI-generated code properly?", "Make it cheaper: one concern per review, decisions shown first, and the code beside every claim. deck gives your agent that format."),
            ("Will this slow us down?", "It moves time earlier. A few minutes deciding before the merge is cheaper than an incident review after it."),
        ],
    },
    {
        "slug": "ai-code-quality",
        "title": "Keeping code quality when an AI agent writes most of it",
        "h1": "Keeping code quality when your agent writes most of it",
        "description": "AI agents write working code fast. Quality slips in the decisions nobody made on purpose. How to keep judgement over a codebase an agent mostly writes.",
        "eyebrow": "AI code quality",
        "body": """
Code written by agents is usually fine line by line. Where quality slips is between the lines: three caches that each made sense alone, a retry added twice, an error swallowed because the test only checked the happy path.

## Quality is a series of decisions

A codebase stays good when the decisions in it were made on purpose by someone who understood the trade-off. With an agent writing most of the code, those decisions still get made. They just get made quickly, inside a task, and approved in bulk.

## Keep judgement where it matters

You do not need to review everything equally. Watch the decisions that compound:

- **New state.** Caches, queues, tables, flags. Each one is forever.
- **Failure handling.** Retries, timeouts, fallbacks, swallowed errors.
- **Boundaries.** What calls what, and who is allowed to.
- **Tests.** Whether they pin behaviour or just the implementation the agent wrote.

## Make the agent argue for its choices

Agents are good at explaining, if you ask for the right shape. The useful shape is a claim, the lines that support it, and the alternative it rejected.

[deck](/) gives your agent that shape. It writes the explanation as a short deck you click through: each sentence lights the code it is about, and when it matters the agent draws the behaviour, like latency before and after or a queue on its busiest day. You push back on the line, and the same agent answers.

The result is a codebase where each decision was shown to someone and chosen. That is most of what quality is.

## A small habit

At the end of any change that adds state or changes failure handling, ask your agent to show you that part before you approve. It takes a minute, and you will catch the second cache before there is a third.
""",
        "faq": [
            ("Is AI-generated code lower quality?", "Not line by line. The risk is decisions made quickly and approved without being understood. Review the decisions and quality holds."),
            ("How do I keep an AI-written codebase maintainable?", "Make sure every decision that adds state or changes failure handling is shown to someone and chosen on purpose."),
            ("Where does deck help with quality?", "It makes your agent show its decisions on the actual lines, so you can agree or push back before they ship."),
        ],
    },
    {
        "slug": "vibe-coding",
        "title": "Vibe coding without losing track of your own system",
        "h1": "Vibe coding without losing track of your system",
        "description": "Vibe coding is fast and fun until you cannot explain your own code. How to keep the speed and still know what you shipped.",
        "eyebrow": "Vibe coding",
        "body": """
Vibe coding is real, and it is fun. You describe what you want, the agent builds it, and you keep going. The cost shows up later: a bug in code you never read, in a design you never chose.

## Keep the speed, keep the plot

You do not have to give up the speed to keep understanding. You need a few moments where you stop and look at what was decided:

- before a change that adds state, like a table, a cache or a queue
- after a change you could not explain to a friend
- before you ship anything that takes money or deletes data

## Look, don't read

Reading every line kills the vibe. Looking at the few lines that matter does not.

[deck](/) is built for those moments. Ask your agent to show you, and it writes a short deck: a few sentences, each with the code it is about lit up beside it, and a small chart when the point is something that moves. You click through it in a minute, ask about whatever looks off, and keep going.

## A lightweight loop

1. Build with your agent as usual.
2. At a natural stopping point, ask it to show you the decisions it made.
3. Push back on anything you would not have chosen.
4. Keep going, knowing what you shipped.
""",
        "faq": [
            ("What is vibe coding?", "Building software mostly by describing what you want to an AI agent and accepting what it writes, often without reading much of the code."),
            ("Is vibe coding bad?", "It is great for speed. It goes wrong when nobody understands the decisions in the result. Short check-ins fix that."),
            ("Does deck slow vibe coding down?", "A deck takes a minute to click through, and only at the moments you choose."),
        ],
    },
    {
        "slug": "learn-unfamiliar-codebase",
        "title": "Learning a codebase you didn't build, with your agent",
        "h1": "Learning a codebase you didn't build",
        "description": "New team, unfamiliar service, the person who built it has left. How to get your agent to walk you through one real request end to end, on the actual code.",
        "eyebrow": "Onboarding",
        "body": """
You have joined the team that owns search, and the person who built it has left. The code is there, the docs are thin, and you are on call next week.

## Follow one real thing, end to end

The fastest way into an unfamiliar system is not reading it top to bottom. It is following one real request from the outside in: what a user does, which service receives it, which lines decide what happens, and what comes back.

Pick a concrete case: "red running shoes under $80". Then follow it.

## Ask for the journey, not the tour

A tour of the directories teaches you where files live. A journey teaches you how the system thinks. Ask your agent for:

1. a map of the parts this request touches
2. the code at each stop, in order
3. the numbers that were chosen (weights, limits, timeouts) and why

## See it as you learn it

[deck](/) lets your agent give you that journey as something you click through. A map of the service, with the request's path drawn through it. The code at each stop, lit sentence by sentence. When you ask why popularity counts for 30 per cent, the answer comes back in the same window, sometimes as a chart of the experiment that chose the number.

You learn the system the way you would from the person who built it, at your own pace, and the questions you ask are pinned to the lines they are about.

## Make it stick

- Follow two or three different requests, not one.
- End each one by writing down what you would change. That is the start of owning it.
""",
        "faq": [
            ("How do I learn a large codebase quickly?", "Follow one real request end to end, from the user's side, through the code that decides what happens. Then follow a second one."),
            ("Can an AI agent help with onboarding?", "Yes, especially if it shows you the code at each step instead of describing it. deck gives it a way to do that."),
            ("Does my code leave my machine?", "No. deck opens your files locally."),
        ],
    },
    {
        "slug": "debug-with-ai-agent",
        "title": "Debugging with an AI agent: see the root cause, not a paragraph",
        "h1": "Debugging with your agent: see the root cause",
        "description": "Your agent found the bug and wrote five paragraphs about it. How to get the root cause shown on the lines, with the timeline of what happened, so you can confirm it.",
        "eyebrow": "Debugging",
        "body": """
Your agent found the bug. Its explanation is five paragraphs long, names six files, and ends with a fix. It might be right. You cannot tell without redoing the investigation yourself.

## A root cause is an argument

A good root cause explanation is short and checkable:

1. **The symptom.** What someone saw, and when.
2. **The surprise.** The one line that does the wrong thing.
3. **The evidence.** Why that line explains the symptom, and nothing else does.
4. **The fix.** And why it removes the cause, not just the symptom.

## Timing bugs need a timeline

Many real bugs are about order: two requests racing for one balance, a cache expiring for everyone at once, a queue filling faster than it drains. A paragraph about a race is very hard to check. A timeline of it is easy.

[deck](/) lets your agent show you a root cause that way. The code sits beside a small page the agent draws, like two cash-outs landing on one wallet milliseconds apart. As each sentence is read, its line lights up and the timeline moves forward, until the money is missing. The next group shows the fix replaying the same moment.

## Confirm before you merge

When the cause is shown on the lines, confirming it takes a minute. When it is not, you either trust it or redo the work. Ask your agent to show you before you accept the fix, and ask what it could not verify. That part is the most useful thing to read.
""",
        "faq": [
            ("How do I check an AI agent's root cause analysis?", "Ask for the one line that does the wrong thing and the evidence that ties it to the symptom, shown on the code. If timing is involved, ask for a timeline."),
            ("Can deck show race conditions?", "Yes. Your agent can draw a small page, like a timeline of two requests, that moves in step with the narration."),
            ("Does deck run tests or reproduce bugs?", "No. It shows what your agent found. Running and verifying is still your agent's and your job."),
        ],
    },
    {
        "slug": "plan-with-ai-agent",
        "title": "Plan a change with your agent before it writes code",
        "h1": "Plan with your agent before it writes code",
        "description": "The cheapest time to disagree with your agent is before the code exists. How to review a plan as code plus consequences, and decide with both numbers in front of you.",
        "eyebrow": "Planning",
        "body": """
The cheapest moment to disagree with your agent is before it writes anything. Once there are four hundred lines, the conversation is about fixing them, not about whether they should exist.

## Review the plan as code plus consequences

A plan you can actually review has three parts:

1. **Today's code.** The lines that will change.
2. **The change.** What they become.
3. **What it buys and what it costs.** In numbers, under real load.

Most plans are written as bullet points. They read well and hide the cost.

## A real example

Checkout is slow because it renders the receipt PDF while the customer waits. The agent proposes moving the render to a queue. The p99 drops from 4.2 seconds to 260 milliseconds.

The cost is in the other number. On Black Friday, orders rise twelve-fold by noon, four workers fall behind, and receipts arrive nine minutes late. Autoscaling the workers keeps that under forty seconds.

That is the decision, and it is only visible with both numbers in front of you.

## Plan in a deck

[deck](/) lets your agent present a plan exactly like that: today's checkout code, the proposed change shown as old lines going and new lines arriving, and a chart of what the change does to latency and to the busiest day of the year. You can [try that exact deck here](/#play).

You decide, comment on the lines you want different, and the agent writes the code with your decision in hand.
""",
        "faq": [
            ("Should I make my AI agent plan before coding?", "For anything that adds state, changes failure handling or touches money, yes. Disagreeing with a plan is much cheaper than rewriting code."),
            ("What should a good plan include?", "The code that will change, what it becomes, and what it costs under real load, ideally with numbers."),
            ("Can I try a planning deck without installing anything?", "Yes. The Black Friday plan runs in the browser on the deck home page."),
        ],
    },
    {
        "slug": "works-with",
        "title": "deck works with Claude Code, Codex, Cursor and Amp",
        "h1": "Works with the agent you already use",
        "description": "deck works with any coding agent that can run a shell command, including Claude Code, Codex, Cursor and Amp. No new model, no API key, no account.",
        "eyebrow": "Compatibility",
        "body": """
deck does not run a model, and it does not replace your agent. The agent you already use writes the deck, in the same session, with everything it just read.

## Any agent with a shell

A deck is written with four shell commands: `deck new`, `deck open`, `deck group` and `deck seal`. Any agent that can run a command can use it. That includes:

- **Claude Code**
- **Codex**
- **Cursor**
- **Amp**

`deck setup` finds the agents on your machine and installs a short skill into each, so they know when a deck is a better answer than a paragraph.

## The catch, for agents with reply hooks

Some agents can run a hook when they finish a reply. For those, setup offers **the catch**: when a reply names two or more `file:line` locations, it is sent back to the agent to become a deck instead. Claude Code supports this today. Agents without hooks still use deck whenever their skill suggests it.

## What you need

- macOS, Linux or Windows
- one of the agents above, or any agent with a shell
- nothing else: no account, no API key, no paid tier

Install it from the [home page](/#install), or read the [docs](/docs/).
""",
        "faq": [
            ("Does deck work with Claude Code?", "Yes. Setup installs its skill, and offers the catch hook, which Claude Code supports."),
            ("Does deck work with Codex, Cursor and Amp?", "Yes. Any agent that can run a shell command can write a deck, and setup installs the skill for each it finds."),
            ("Do I need an API key or account?", "No. deck is free and open source, and uses the agent you already have."),
        ],
    },
]

GUIDES += [
    {
        "slug": "coderabbit-alternative",
        "title": "Looking for a CodeRabbit alternative? A different kind of AI code review",
        "h1": "A different kind of CodeRabbit alternative",
        "description": "CodeRabbit and similar bots comment on your pull request after the fact. deck is the step before: your own agent shows you each decision on the lines, and you decide. Free and open source.",
        "eyebrow": "CodeRabbit alternative",
        "body": """
If you are searching for a CodeRabbit alternative, you probably have a specific complaint. The most common ones are too many comments, comments that miss the point, or a feeling that the bot reviewed the code and nobody on the team did.

Tools like CodeRabbit, Greptile, GitHub Copilot code review, Graphite's reviewer, Qodo and Cursor BugBot all do a similar job: they read a pull request and leave comments on it. They are useful for catching things. They are not designed to make *you* understand what changed.

## A different question

A review bot answers "is anything wrong with this diff?". The question that is usually harder, when an agent wrote the change, is "what did we just decide, and do I agree?".

[deck](/) answers that one. It is not a bot and it is not a service. It is a window on your machine that the agent you already use (Claude Code, Codex, Cursor, Amp) opens to show you its work: one claim at a time, with the lines that prove it lit beside it, and a chart when the point is about behaviour. You push back on the line; the same agent answers with its full context.

## How they compare

- **When it happens.** A review bot runs after the PR is open. deck happens before, while the agent still holds everything it just read.
- **Who explains.** A bot explains its own findings. With deck, the agent that wrote the code explains the code, and answers your questions.
- **What you get.** A bot gives you comments to triage. deck gives you the decisions to agree with or change.
- **Cost.** deck is free and open source, with no account and no seats.

## Use both, if you like

Nothing stops you running a review bot on the PR and using deck before it. Many teams will want both: a bot to catch slips, and a way for the person approving to actually understand the change.

[Try a real deck in your browser](/#play), or [install it](/#install).
""",
        "faq": [
            ("Is deck a CodeRabbit competitor?", "Not directly. CodeRabbit comments on pull requests. deck lets your own agent show you its changes before you approve. You can use both."),
            ("Does deck integrate with GitHub?", "No integration is needed. It runs on your machine with the agent you already use; your PR process stays as it is."),
            ("How much does deck cost?", "Nothing. It is free and open source under Apache-2.0."),
        ],
    },
    {
        "slug": "ai-code-review-tools",
        "title": "AI code review tools compared: bots that comment, and agents that show you",
        "h1": "AI code review tools, compared honestly",
        "description": "CodeRabbit, Greptile, GitHub Copilot code review, Graphite, Qodo, Cursor BugBot and deck do different jobs. What each kind of tool is for, and how they fit together.",
        "eyebrow": "AI code review tools",
        "body": """
"AI code review" now covers two quite different kinds of tool. Picking well starts with knowing which job you need done.

## Kind one: bots that review pull requests

CodeRabbit, Greptile, GitHub Copilot code review, Graphite's reviewer, Qodo Merge and Cursor BugBot read a pull request and leave comments. They differ in how much of the codebase they consider, how noisy they are, which platforms they support, and what they cost. They are good at finding slips a tired human would miss.

What they do not do is transfer understanding. After a bot review, the code may be better, and the person approving it may still not know what was decided.

## Kind two: your agent showing you its work

[deck](/) is the second kind. It does not review anything by itself. It gives the agent that wrote the code (Claude Code, Codex, Cursor, Amp, anything with a shell) a way to show you the change: one claim at a time, the lines that prove it lit beside it, charts for behaviour over time, and a place to disagree on the exact line. The same agent answers, in the same session.

## Which do you need?

- **"We miss bugs in review."** A review bot helps.
- **"Nobody understands what the agent changed."** You need the agent to show its decisions. That is deck.
- **"Our PRs are too big to review."** Split them into concerns; deck can walk each one as a chapter.
- **"The bot's comments are noise."** Fewer, better comments help. So does reviewing decisions instead of lines.

## They fit together

A sensible setup for a team using coding agents: the agent shows you its decisions before the PR (deck), a bot checks the PR for slips, and a human approves knowing what was decided.
""",
        "faq": [
            ("What is the best AI code review tool?", "It depends on the job. Bots like CodeRabbit or Greptile catch mistakes in pull requests. deck helps you understand and decide on what your agent changed. Many teams want both."),
            ("Is deck an AI code review bot?", "No. It does not comment on its own. It is how your agent shows you its work so you can review it."),
            ("Is deck free?", "Yes, free and open source under Apache-2.0."),
        ],
    },
    {
        "slug": "ai-code-review-noise",
        "title": "AI code review noise: when the bot leaves 72 comments",
        "h1": "When AI code review becomes noise",
        "description": "Review bots that comment on everything train people to ignore them. Why AI code review gets noisy, and how to get signal: fewer claims, shown on the code, that you decide on.",
        "eyebrow": "Review fatigue",
        "body": """
A familiar story: an architectural pull request, seventy-two comments, one human in the thread. Nobody reads them all. After a few weeks, nobody reads most of them.

## Why it gets noisy

Review bots flag what *could* be wrong, not what is. Style nits, vague "consider adding tests", rewrites that do not fix a bug. Each one costs a moment of attention, and attention is the thing a review is short of.

The result is alert fatigue. The real problem, when there is one, arrives looking exactly like the forty comments before it.

## Signal is a short argument

What a reviewer needs is the opposite of a pile of comments: a few claims about the change, ordered by importance, each with the evidence beside it, and a clear question at the end.

That is a deck. With [deck](/), the agent that made the change shows you three or four groups, not seventy findings. Each one is a claim with its lines lit. You read the argument in a few minutes, and you disagree where it matters.

## Practical ways to cut the noise

- Turn off style comments in bots; let a formatter handle style.
- Ask for decisions first: what was chosen, and what was the alternative.
- Review one concern at a time.
- Keep comments for things a human must decide.
""",
        "faq": [
            ("Why are AI code reviews so noisy?", "Most bots flag anything that might be wrong rather than proving that it is, so real issues get buried among nits and maybes."),
            ("How do I reduce AI code review noise?", "Turn off style comments, review one concern at a time, and ask for the few decisions that matter instead of every finding."),
            ("Does deck add more comments to my PR?", "No. It does not touch your PR. It is where your agent shows you its decisions before you approve."),
        ],
    },
    {
        "slug": "comprehension-debt",
        "title": "Comprehension debt: the cost of code nobody on the team understands",
        "h1": "Comprehension debt, and how to pay it down",
        "description": "Comprehension debt (or cognitive debt) is the gap between the code your agent wrote and what your team understands. Why it grows, why it bites, and how to pay it down as you go.",
        "eyebrow": "Comprehension debt",
        "body": """
Technical debt is code you know is wrong. **Comprehension debt** is code that might be fine, but nobody understands it well enough to say. Some people call it cognitive debt. Either way, it grows fastest when an agent writes the code.

## How it builds up

Every change your agent makes is reasonable. You approve it, because it works and you are busy. The understanding you would have built by writing it never gets built. A month later, a bug lands in that code, and nobody can reason about it quickly. That delay is the interest on the debt.

## Why it bites

- Incidents take longer, because nobody holds the model of the system.
- Changes get riskier, because nobody knows what relies on what.
- The agent itself gets worse: you cannot correct what you do not understand, so its mistakes compound.

## Pay it down as you go

The cheapest time to understand a change is the moment it is made, while the agent still holds the whole context. A few minutes then saves hours later.

[deck](/) makes that moment cheap. Your agent shows you the change as a short deck: the situation, the lines that carry it, and the decision, each sentence lighting the code it is about. You ask where it does not add up, and the answer comes back on the lines. You end up understanding the decision, not just having approved it.

## A rule of thumb

If you could not explain a change to a teammate in two minutes, ask your agent to show it to you before you merge it.
""",
        "faq": [
            ("What is comprehension debt?", "The gap between the code in your system and what your team understands of it. It grows quickly when AI agents write code that gets approved without being understood."),
            ("Is comprehension debt the same as technical debt?", "No. Technical debt is known shortcuts. Comprehension debt is code that may be fine but nobody can reason about."),
            ("How does deck help with comprehension debt?", "It has your agent show you each change on the actual lines while it still has the context, so understanding is built at the moment it is cheapest."),
        ],
    },
    {
        "slug": "code-review-bottleneck",
        "title": "Code review is the bottleneck now that AI writes the code",
        "h1": "Review is the bottleneck now",
        "description": "Agents write code faster than anyone can read it, so the pressure moves to review. Why more code means less understanding, and how to review at the speed agents write.",
        "eyebrow": "The review bottleneck",
        "body": """
Writing code got fast. Reviewing it did not. A teammate opens an 800-line pull request written in three hours instead of three days, and someone has to approve it before lunch.

## The pressure moved

When generation gets cheap, the expensive part is everything after it: understanding, deciding, owning. Teams feel this as more pull requests, bigger diffs, and approvals that mean less each week.

## Faster reading will not fix it

You cannot read 800 lines faster. You can read fewer of them: the decisions, with their evidence beside them, and skip the consequences.

That only works if someone tells you where the decisions are. The person best placed to do that is the one who made them, which is now often the agent.

## Let the author do the explaining

[deck](/) gives your agent a way to hand you its decisions instead of its diff. One claim per group, the lines lit beside each sentence, a chart when the claim is about load or timing. You review the argument in minutes and push back on the lines that matter. The agent answers with everything it knew when it wrote the code.

Review stays a human decision. It just stops being a reading contest.
""",
        "faq": [
            ("Why is code review a bottleneck with AI coding tools?", "Agents generate code faster than people can read it, so understanding and approving becomes the slow step."),
            ("How do I review AI-written pull requests faster?", "Review the decisions, not every line, and have the author, human or agent, show you where they are and why."),
            ("Does deck speed up review?", "It makes the review smaller: your agent shows you its few decisions with the evidence, instead of handing you the whole diff."),
        ],
    },
]
