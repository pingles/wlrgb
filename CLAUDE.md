# Work Louder - Creator Micro 2

This repository creates a tool suitable for dynamically controlling the RGB highlighting for the Work Louder Creator Micro v2 keyboard.

The tool integrates via Claude Code's hooks:


* When waiting for user input, the RGB lighting will blink white
* While thinking/working, the RGB lighting will cycle around orange in the same Claude Code colours.
* When finished, and not waiting, colours return to the pre-existing style.

## How it works

A command-line tool (built in Rust) that is invoked via a hook. This will send messages/control the Work Louder - Input application that can control the keyboard.
