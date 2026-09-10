# saycmd

`saycmd` turns a tagged natural-language request in Zsh into a command or multi-line
script that you can review before running.

```text
% ？查询当前路径
% pwd
```

The first Enter calls the model and replaces the current editor buffer. Nothing is
executed until you review the result and press Enter again.

## Build and install

```sh
cargo install --path .
mkdir -p ~/.config/saycmd
```

Create `~/.config/saycmd/config.toml`:

```toml
base_url = "https://api.openai.com/v1"
model = "your-model-name"
api_key_env = "OPENAI_API_KEY"
timeout_seconds = 30
prefix = "？"
explain = false
```

Keep the API key in the named environment variable:

```sh
export OPENAI_API_KEY="..."
```

For an OpenAI-compatible local endpoint that requires no key, set
`api_key_env = ""`. Environment variables override the file:

- `SAYCMD_BASE_URL`
- `SAYCMD_MODEL`
- `SAYCMD_API_KEY` (direct key override)
- `SAYCMD_API_KEY_ENV`
- `SAYCMD_TIMEOUT_SECONDS`
- `SAYCMD_PREFIX`
- `SAYCMD_EXPLAIN` (`true` or `false`)

Enable the Zsh integration by adding this line to `.zshrc`:

```zsh
eval "$(saycmd init zsh)"
```

Restart Zsh, or evaluate that line in the current session. The default tag is the
full-width question mark `？`. Set `prefix = "?"` in `config.toml` to change it,
then reload the integration. Empty or whitespace-only values use the default.
You can also override the configured prefix with an environment variable:

```zsh
export SAYCMD_PREFIX='?'
eval "$(saycmd init zsh)"
```

Run `saycmd doctor` to check configuration, Zsh, credentials, and the provider's
`/models` endpoint.

Set `explain = true` to show the meaning of each command argument below the
editable command. The default `false` leaves the message area blank after generation.
This setting is read on each request; no shell reload is needed after changing it.
While generating, a spinner and elapsed seconds appear below the input line.

## Follow-up revisions

Use `??` to revise the most recently generated command in the current terminal:

```text
?list files
→ ls
??sort by modification time
→ ls -t
??include hidden files too
→ ls -ta
```

Replace the command in the editor with `??` followed by your revision, then press
Enter. You can also follow up after executing the command. No shortcut is required.
Each result remains editable and requires another Enter to execute.

`??` is reserved for follow-ups, regardless of the configured initial prefix.
The previous command, original request and up to seven latest revisions are kept
only in the current shell's memory and sent with follow-up requests. This does not
read shell history or track manual edits to commands. A successful new request using
the initial prefix resets the context; failed requests preserve it. Without a previous
command, `??` displays a message asking you to generate one first.

## Direct use

```sh
saycmd translate --shell zsh --cwd "$PWD" -- "find large files modified this week"
```

Use `--explain` or `--explain=false` to override the explanation setting for one request.
Standard output contains only the generated command. Explanations (when enabled)
and errors go to standard error.
The chat endpoint must support JSON Output (`response_format: {"type":"json_object"}`).
Requests include a JSON example; returned fields and Zsh syntax are validated locally.
Requests explicitly disable thinking (`thinking: {"type": "disabled"}`) and streaming (`stream: false`).
The model receives the prompt, current directory, OS/architecture, shell, and—when
applicable—the Git root, branch, and dirty boolean. It does not receive command
history, environment variables, filenames, directory listings, or file contents.

## Safety model

Generated commands are untrusted input. `saycmd` checks Zsh syntax without executing
the result, but syntax validity does not make a command safe. Always inspect the
buffer before pressing Enter again.
