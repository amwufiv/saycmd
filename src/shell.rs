/// Quote the configured prefix as literal shell data.
pub fn zsh_init(prefix: &str) -> String {
    let quoted = format!("'{}'", prefix.replace('\'', "'\\''"));
    ZSH_INIT.replace("__SAYCMD_PREFIX__", &quoted)
}

const ZSH_INIT: &str = r#"# saycmd zsh integration
if (( ! ${+widgets[saycmd-original-accept-line]} )); then
  zle -A accept-line saycmd-original-accept-line
fi

_saycmd_accept_line() {
  local prefix=__SAYCMD_PREFIX__
  prefix="${SAYCMD_PREFIX:-$prefix}"
  local followup_prefix="${prefix}${prefix}"
  local followup=0
  if [[ -n "$prefix" && "$BUFFER" == "$followup_prefix"* ]]; then
    followup=1
    prefix="$followup_prefix"
  elif [[ -z "$prefix" || "$BUFFER" != "$prefix"* ]]; then
    zle saycmd-original-accept-line
    return
  fi

  local request="${BUFFER[${#prefix}+1,-1]}"
  request="${request#"${request%%[![:space:]]*}"}"
  if [[ -z "$request" ]]; then
    zle -M "saycmd: prompt is empty"
    return 1
  fi

  local -a previous_args=()
  if (( followup )); then
    if [[ -z "${_saycmd_last_command-}" ]]; then
      zle -M "saycmd: no previous command; generate one first"
      return 1
    fi
    previous_args+=("--previous-command=$_saycmd_last_command")
    local previous_request
    for previous_request in "${_saycmd_requests[@]}"; do
      previous_args+=("--previous-request=$previous_request")
    done
  fi

  local original="$BUFFER"
  local generated
  local saycmd_tmp
  if ! saycmd_tmp="$(command mktemp -d "${TMPDIR:-/tmp}/saycmd.XXXXXXXXXX")"; then
    zle -M "saycmd: cannot create temporary directory"
    return 1
  fi
  # Suppress background job notifications only for the duration of this widget.
  setopt localoptions no_monitor no_notify
  local saycmd_pid
  local -i saycmd_exit_code=0 frame=1 started=$SECONDS
  local -a frames=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
  {
    command saycmd translate --zle --shell zsh --cwd "$PWD" "${previous_args[@]}" -- "$request" >"$saycmd_tmp/output" 2>&1 &
    saycmd_pid=$!
    while kill -0 "$saycmd_pid" 2>/dev/null; do
      zle -R "${frames[$frame]} generating… $(( SECONDS - started ))s"
      frame=$(( frame % ${#frames} + 1 ))
      command sleep 0.1
    done
    if wait "$saycmd_pid"; then
      saycmd_exit_code=0
    else
      saycmd_exit_code=$?
    fi
    saycmd_pid=''
    generated="$(<"$saycmd_tmp/output")"
  } always {
    if [[ -n "$saycmd_pid" ]]; then
      kill "$saycmd_pid" 2>/dev/null
      wait "$saycmd_pid" 2>/dev/null
    fi
    command rm -rf -- "$saycmd_tmp"
  }
  if (( saycmd_exit_code != 0 )); then
    BUFFER="$original"
    generated="${generated#saycmd: }"
    zle -M "saycmd: ${generated//$'\n'/ }"
    return $saycmd_exit_code
  fi

  local explanation=""
  if [[ "$generated" == *$'\0'* ]]; then
    explanation="${generated#*$'\0'}"
    generated="${generated%%$'\0'*}"
  fi
  typeset -g _saycmd_last_command="$generated"
  if (( ! followup )); then
    typeset -ga _saycmd_requests=()
  fi
  _saycmd_requests+=("$request")
  # Keep the original request and the seven latest revisions.
  if (( ${#_saycmd_requests} > 8 )); then
    _saycmd_requests=("${_saycmd_requests[1]}" "${(@)_saycmd_requests[-7,-1]}")
  fi
  BUFFER="$generated"
  CURSOR=${#BUFFER}
  zle -M -- "$explanation"
}

zle -N accept-line _saycmd_accept_line
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent_and_does_not_execute_generated_command() {
        assert!(ZSH_INIT.contains("widgets[saycmd-original-accept-line]"));
        assert!(ZSH_INIT.contains("BUFFER=\"$generated\""));
        assert!(!ZSH_INIT.contains("eval \"$generated\""));
    }
}
