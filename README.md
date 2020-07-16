# RustPoker

Fast range vs. range equity calculation for poker written in rust

## Hand Evaluator
 - Evaluates hands with any number of cards from 0 to 7
 - Higher score is better

## Equity Calculator
 - Runs a multithreaded monte-carlo simulation to calculate range vs range equities
 - Supports up to 6 players

# Credit

The hand evaluator and equity calculator library is a rust rewrite of **zekyll's** C++ equity calculator, [OMPEval](https://github.com/zekyll/OMPEval)
