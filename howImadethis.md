//Got some architectural shel docs from W3resources 

//
got this as core view and event 
Keyboard ──► crossterm (raw events) ──► PTY master ──► bash
Screen   ◄── PTY master ◄──────────── shell output


cargo run
   ↓
Rust terminal program
   ↓
PTY created
   ↓
/bin/bash launched inside PTY
   ↓
bash prints startup messages
   ↓
output forwarded back to your terminal
