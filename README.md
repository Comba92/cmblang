# Running
This project requires the C3 compiler.
To run the REPL:
```bash
  c3c run
```

# Language Specifications

This is a simple toy language, it can define variables of type number, bool, and null.
Numbers are always rational numbers. Bools can either be 'true' or 'false'. Null can only be 'null'.
To declare a variable:
```
  var variable_name = value_literal
```

Numbers implement the four basic operations (+, -, *, /).
Bools implement the basic logical operations (and, or, not).
Values of the same type can be compared with equality relations (==, !=).
Numbers can also be compared with greater than relations (>, >=), and less than relations (<, <=).
Using the wrong operands will result in a runtime error.
Nulls can't be used in any operation, and can only be compared with an equality.
Expressions can be grouped with parenthesis ( and ).
The REPL will always print the result of an expression.

Single line comments start with //.

It is possible to create scopes with { and }. A variable defined within a scope will be destroyed at the end of it.
Variables defined in outer scopes will be visible in inner scopes.
Shadowing is applied if a variable of the same name in an outer scope is defined in an inner scope.
```
var a = 5
{
  // a is shadowed
  var a = 10
  var b = 2
  print a + b // will print 12
}
// b no longer exists here
// a is now 5
```

If/else conditions and while loops are avaible. Braces are always required. 
As usual:
```
  if (condition) {
    // if branch here
  } else {
    // else branch here
  }

  while (condition) {
    // while body here
  }
```

# TODO
- REPL Qol
- Run a program from a file
- Better runtime errors
- String type
- Lists and Maps
- Functions
- Better memory mangament
