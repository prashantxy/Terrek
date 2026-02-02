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


making it for all session would kill lots of memory so rather make it for a particular session at max 3 and once the user would want to store more they can replace the shit

PTY Thread  ─┐
              ├──>  channel  ───>  DB Thread  ──> SQLite
Main Thread ──┘


also storing all the things in one single file would cost me lots of data redundancy and could create a possible deadlock condition .

how?????????



Keyboard → terminal → recorder
PTY → recorder
recorder → channel → db/worker → SQLite
commands → db/worker (queries)

this could ne the possible architecture for the current setup??????????



Terrek = PTY terminal + persistent memory engine.