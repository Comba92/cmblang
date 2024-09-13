module parser;

import lexer;
import ast;

import std::io;

fault ParserErr {
    INVALID_TOKEN,
    UNTERMINATED_GROUPING,
    UNTERMINATED_BLOCK,
    INVALID_ASSIGN_TARGET,
    MISSING_IF_PARENTHESIS,
    UNTERMINATED_IF_CONDITION,
    INVALID_VAR_NAME,
    MISSING_VAR_INITIALIZATION,
    MISSING_COMMA_IN_LISTING,
    UNTERMINATED_LIST_LITERAL,
    UNTERMINATED_INDEXING,
}

struct Parser {
    usz index;
    TokenList tokens;
    StmtList statements;
    bool panicked;
}
fn bool Parser.is_at_end(Parser self) {
    return self.index >= self.tokens.len();
}
fn Token* Parser.peek(Parser self) {
    if ( self.is_at_end() ) { 
        return mem::new(Token, Token {
            .kind = TokenKind.INVALID
        });
    }
    return self.tokens.get(self.index);
}
fn Token* Parser.consume(Parser* self) {
    if (self.is_at_end()) {
        return mem::new(Token, Token {
            .kind = TokenKind.INVALID
        });
    }
    Token* token = self.peek();
    self.advance();
    return token;
}
fn void Parser.advance(Parser* self) {
    self.index += 1;
}
fn bool Parser.match(Parser* self, TokenKind target) {
    return self.match_any({target});
}
fn bool Parser.match_any(Parser* self, TokenKind[] targets) {
    return self.peek().kind.match_any(targets);
}
fn Token* Parser.expect(Parser* self, TokenKind target, ParserErr err) {
    return self.expect_any({target}, err);
}
fn Token* Parser.expect_any(Parser* self, TokenKind[] targets, ParserErr err) {
    if (!self.peek().kind.match_any(targets)) self.panic(err);
    return self.consume();
}

fn Expr* Parser.panic(Parser* self, ParserErr err) {
    String msg;
    if (self.is_at_end()) { msg = string::new_format("Parser error %s at end", err); }
	else { 
        // Token* current = self.tokens[self.index-1];
        Token* current = self.peek();
        msg = string::new_format("Parser error %s at token '%s', line %d, col %d", 
                                err, 
                                current.lexeme,
                                current.loc.line,
                                current.loc.col);
    }
    
    io::eprintn(msg);
    self.index = self.tokens.len();
    self.panicked = true;

    return {};
}

fn StmtList parse(TokenList tokens) {
    Parser parser = { .tokens = tokens };

    while (!parser.is_at_end()) {
        Stmt* res = stmt(&parser);
        parser.statements.push(res);
    }

    if (parser.panicked) return StmtList {};
    return parser.statements;
}

fn Stmt* stmt(Parser* parser) {
    if (parser.match(TokenKind.VAR_KEY)) {
        return mem::new(Stmt, *vars(parser));
    }
    if (parser.match(TokenKind.PRINT_KEY)) {
        return mem::new(Stmt, *print(parser));
    }
    if (parser.match(TokenKind.LEFTBRACE)) {
        return mem::new(Stmt, *block(parser));
    }
    if (parser.match(TokenKind.IF_KEY)) {
        return mem::new(Stmt, *ifs(parser));
    }
    if (parser.match(TokenKind.WHILE_KEY)) {
        return mem::new(Stmt, *whiles(parser));
    }

    return mem::new(Stmt, Stmt {.kind = StmtKind.EXPR, .expr = expr(parser)});
}

fn Stmt* print(Parser* parser) {
    parser.consume();
    Expr* value = expr(parser);

    PrintStmt* print = mem::new(PrintStmt, PrintStmt {.expr = value});
    return mem::new(Stmt, Stmt {.kind = StmtKind.PRINT, .print = print});
}

fn Stmt* vars(Parser* parser) {
    parser.consume();
    Token* name = parser.expect(TokenKind.IDENTIFIER, ParserErr.INVALID_VAR_NAME);
    parser.expect(TokenKind.EQUAL, ParserErr.MISSING_VAR_INITIALIZATION);
    Expr* value = expr(parser);

    VarStmt* vars = mem::new(VarStmt, VarStmt {.name = name, .value = value});
    return mem::new(Stmt, Stmt {.kind = StmtKind.VAR, .vars = vars});
}

fn Stmt* block(Parser* parser) {
    Token* left = parser.consume();
    StmtList body = {};

    while (!parser.is_at_end()) {
        if (parser.match(TokenKind.RIGHTBRACE)) { break; }
        body.push(stmt(parser));
    }

    Token* right = parser.expect(TokenKind.RIGHTBRACE, ParserErr.UNTERMINATED_BLOCK);
    BlockStmt* block = mem::new(BlockStmt, BlockStmt {.left = left, .right = right, .body = body});
    return mem::new(Stmt, Stmt {.kind = StmtKind.BLOCK, .block = block});
}

fn Stmt* ifs(Parser* parser) {
    parser.consume();
    parser.expect(TokenKind.LEFTPAREN, ParserErr.MISSING_IF_PARENTHESIS);
    Expr* condition = expr(parser);
    parser.expect(TokenKind.RIGHTPAREN, ParserErr.UNTERMINATED_IF_CONDITION);

    Stmt* ifs_branch = stmt(parser);
    Stmt* else_branch = null;
    if (parser.match(TokenKind.ELSE_KEY)) {
        parser.consume();
        else_branch = stmt(parser); 
    }

    IfStmt* ifs = mem::new(IfStmt, IfStmt {.condition = condition, .if_branch = ifs_branch, .else_branch = else_branch});
    return mem::new(Stmt, Stmt {.kind = StmtKind.IF, .ifs = ifs});
}

fn Stmt* whiles(Parser* parser) {
    parser.consume();
    parser.expect(TokenKind.LEFTPAREN, ParserErr.MISSING_IF_PARENTHESIS);
    Expr* condition = expr(parser);
    parser.expect(TokenKind.RIGHTPAREN, ParserErr.UNTERMINATED_IF_CONDITION);

    Stmt* body = stmt(parser);
    WhileStmt* whiles = mem::new(WhileStmt, WhileStmt {.condition = condition, .body = body});
    return mem::new(Stmt, Stmt {.kind = StmtKind.WHILE, .whiles = whiles});
}

fn Expr* expr(Parser* parser) {
    return assign(parser);
}

fn Expr* assign(Parser* parser) {
    Expr* left = logical_or(parser);

    if (parser.match(TokenKind.EQUAL)) {
        if (left.kind != ExprKind.LITERAL
        || left.literal.kind != TokenKind.IDENTIFIER) {
            return parser.panic(ParserErr.INVALID_ASSIGN_TARGET);
        }

        Token* equal = parser.consume();
        Expr* right = assign(parser);
        AssignExpr assign = AssignExpr {.equal = equal, .identifier = left, .value = right};
        return mem::new(Expr, Expr {.kind = ExprKind.ASSIGN, .assign = assign});
    }
    
    return left;
}

fn Expr* logical_or(Parser* parser) {
    TokenKind[*] ops = {TokenKind.OR};
    return binary(parser, &ops, &logical_and);
}

fn Expr* logical_and(Parser* parser) {
    TokenKind[*] ops = {TokenKind.AND};
    return binary(parser, &ops, &equality);
}

fn Expr* equality(Parser* parser) {
    TokenKind[*] ops = {
        TokenKind.BANGEQUAL,
        TokenKind.EQUALEQUAL
    };
    return binary(parser, &ops, &comparison);
}

fn Expr* comparison(Parser* parser) {
    TokenKind[*] ops = {
        TokenKind.LESS,
        TokenKind.LESSEQUAL,
        TokenKind.GREATER,
        TokenKind.GREATEREQUAL,
    };
    return binary(parser, &ops, &term);
}

fn Expr* term(Parser* parser) {
    TokenKind[*] ops = {TokenKind.ADD, TokenKind.SUB};
    return binary(parser, &ops, &factor);
}

fn Expr* factor(Parser* parser) {
    TokenKind[*] ops = {
        TokenKind.MUL,
        TokenKind.DIV,
    };
    return binary(parser, &ops, &unary);
}

def GrammarFn = fn Expr*(Parser*);
fn Expr* binary(Parser* parser, TokenKind[] ops, GrammarFn next) {
    Expr* expr = next(parser);
    
    while (!parser.is_at_end()) {
        if (!parser.match_any(ops)) { break; }
        Token* op = parser.consume();

        Expr* right = next(parser);
        BinaryExpr binary = BinaryExpr {.left = expr, .op = op, .right = right};

        expr = mem::new(Expr, Expr {.kind = ExprKind.BINARY, .binary = binary});
    }

    return expr;
}

fn Expr* unary(Parser* parser) {
    TokenKind[*] ops = {
        TokenKind.BANG, 
        TokenKind.NOT,
        TokenKind.SUB,
        TokenKind.HASH,
    };

    if (!parser.match_any(&ops)) {
        return primary(parser);
    }

    Token* op_token = parser.consume();
    Expr* right = unary(parser);
    UnaryExpr unary = UnaryExpr {.op = op_token, .right = right};

    return mem::new(Expr, Expr {.kind = ExprKind.UNARY, .unary = unary});
}

fn Expr* primary(Parser* parser) {
    Token* token = parser.consume();

    switch (token.kind) {
        case LEFTPAREN: {
            Token* left = token;
            Expr* e = expr(parser);
            Token* right = parser.expect(TokenKind.RIGHTPAREN, ParserErr.UNTERMINATED_GROUPING);
            GroupingExpr group = {.left = left, .expr = e, .right = right};
            return mem::new(Expr, Expr {.kind = ExprKind.GROUPING, .grouping = group});
        }
        case LEFTBRACKET: {
            // empty list
            if (parser.match(TokenKind.RIGHTBRACKET)) {
                Token* end = parser.consume();
                ListExpr list = ListExpr {.start = token, .end = end, .exprs = {}};
                return mem::new(Expr, Expr {.kind = ExprKind.LIST, .list = list});
            }

            ExprList exprs;
            while (!parser.is_at_end()) {
                exprs.push(expr(parser));

                if (parser.match(TokenKind.RIGHTBRACKET)) {
                    Token* end = parser.consume();
                    ListExpr list = ListExpr {.start = token, .end = end, .exprs = exprs};
                    return mem::new(Expr, Expr {.kind = ExprKind.LIST, .list = list});
                }

                parser.expect(TokenKind.COMMA, ParserErr.MISSING_COMMA_IN_LISTING);
            }

            return parser.panic(ParserErr.UNTERMINATED_LIST_LITERAL);
        }
        case NULL: nextcase;
        case BOOL: nextcase;
        case NUMBER: nextcase;
        case STRING: nextcase;
        case IDENTIFIER: { 
            // List indexing
            if (parser.match(TokenKind.LEFTBRACKET)) {
                parser.consume();
                Expr* idx = expr(parser);
                parser.expect(TokenKind.RIGHTBRACKET, ParserErr.UNTERMINATED_INDEXING);
                IndexingExpr indexing = {.identifier = token, .index = idx};
                return mem::new(Expr, Expr {.kind = ExprKind.INDEXING, .indexing = indexing});
            }

            return mem::new(Expr, Expr {.kind = ExprKind.LITERAL, .literal = token});
        }

        default: {
            return parser.panic(ParserErr.INVALID_TOKEN);
        }
    }
}