
(
  program
    . (redirected_statement
        body: (command
                name: (command_name (word) @apply_name) .)
        (#any-of? @apply_name "apply_patch" "applypatch")
        redirect: (heredoc_redirect
                    . (heredoc_start)
                    . (heredoc_body) @heredoc
                    . (heredoc_end)
                    .))
    .)

(
  program
    . (redirected_statement
        body: (list
                . (command
                    name: (command_name (word) @cd_name) .
                    argument: [
                      (word) @cd_path
                      (string (string_content) @cd_path)
                      (raw_string) @cd_raw_string
                    ] .)
                "&&"
                . (command
                    name: (command_name (word) @apply_name))
                .)
        (#eq? @cd_name "cd")
        (#any-of? @apply_name "apply_patch" "applypatch")
        redirect: (heredoc_redirect
                    . (heredoc_start)
                    . (heredoc_body) @heredoc
                    . (heredoc_end)
                    .))
    .)
