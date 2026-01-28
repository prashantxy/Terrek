//Got some architectural shel docs from W3resources 

//
got this as core view and event 
Keyboard ──► crossterm (raw events) ──► PTY master ──► bash
Screen   ◄── PTY master ◄──────────── shell output
