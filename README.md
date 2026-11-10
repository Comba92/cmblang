# Type Inference
We use a bidirectional type inference system.
We have an infer function which guesses types, and a check function, which does UNIFICATION.
Unification algorithm uses a Union-Find data structure to solve type CONSTRAINTS of the form (?T -> concreteType).
The Union-Find has 3 methods.
- Make set: this generates a new fresh type variable.
- Find (or normalize): this finds the representative given an id. In our cases, it traverses the contraints chain until we either get an unresolved type variable or a solved concrete type.
- Union (or unify): this takes two types, compares them, and returns an error if they are different. In the special case one of the two types is a type variable, and the other is a solved type, we make a SUBSTITUTION and store the solved type as the set representative for the type variable. We can now find the real type using find, given the type variable (or any other type variable in the constraints chain).

Variable declarations either have an annotation, or the type is inferred from its first assignment. 
Functions are always annotated.
If a function is generic, at declaration, generic parameters get assigned to special Generic types.
At each call site, the function is instantiated. Instantiation substitutes every generic type with an unknown, temporary fresh type variable.
Each fresh variable is then unified with the actual call argument type. The return type should also have been inferred at this time, and it is normalized (looked up in the union-find) to its type.
Variable lookup also requires instantiation. If a generic function is assigned to a variable, we have to instantiate it and assign to it a concrete type once.
