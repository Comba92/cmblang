# Running
This project requires the C3 compiler.
To run the REPL:
```bash
  c3c run
```

# Language Specifications

This is a simple toy language, it can define variables of type number, bool, and null.
```
  variable_name = value
```
Numbers implement the four basic operations (+, -, *, /). Bools implement the basic logical operations (and, or, not).
Values of the same type can be compared with equality relations (==, !=), greater than relations (>, >=), and less than relations (<, <=).
Using the wrong operands will result in a runtime error.
Nulls can't be used in any operation, and can only be compared with an equality.

It is possible to create scopes with { and }. A variable defined within a scope will be destroyed at the end of it.
Shadowing is applied on defined variables.
```
a = 5
{
  // a is shadowed
  a = 10
  b = 2
  a + b // will print 12
}
// b no longer exists here
// a is now 5
```

# TODO
- Run a program from a file
- If/else conditions
- While loops
- Better runtime errors
- String type
- Lists and Maps
- Functions
- Better memory mangament
