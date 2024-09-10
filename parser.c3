module parser;

import lexer;
import ast;

import std::io;
import std::collections::list;
def StmtList = List(<Stmt*>);

fault ParserErr {
    INVALID_TOKEN,
    UNTERMINATED_GROUPING,
    UNTERMINATED_BLOCK,
    INVALID_ASSIGN_TARGET
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

fn void Parser.panic(Parser* self, ParserErr err) {
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
    if (parser.match(TokenKind.LEFTBRACE)) {
        return mem::new(Stmt, *block(parser));
    }

    return mem::new(Stmt, Stmt {.kind = StmtKind.EXPR, .expr = *expr(parser)});
}

fn Stmt* block(Parser* parser) {
    Token* left = parser.consume();
    StmtList body = {};

    while (!parser.is_at_end()) {
        if (parser.match(TokenKind.RIGHTBRACE)) { break; }
        body.push(stmt(parser));
    }

    Token* right = parser.expect(TokenKind.RIGHTBRACE, ParserErr.UNTERMINATED_BLOCK);
    BlockStmt block = BlockStmt {.left = left, .right = right, .body = body};
    return mem::new(Stmt, Stmt {.kind = StmtKind.BLOCK, .block = block});
}

fn Expr* expr(Parser* parser) {
    return assign(parser);
}

fn Expr* assign(Parser* parser) {
    Expr* left = equality(parser);

    if (parser.match(TokenKind.EQUAL)) {
        if (left.kind != ExprKind.LITERAL
        || left.literal.kind != TokenKind.IDENTIFIER) {
            parser.panic(ParserErr.INVALID_ASSIGN_TARGET);
            return mem::new(Expr, Expr {});
        }

        Token* equal = parser.consume();
        Expr* right = assign(parser);
        AssignExpr assign = AssignExpr {.equal = equal, .identifier = left, .value = right};
        return mem::new(Expr, Expr {.kind = ExprKind.ASSIGN, .assign = assign});
    }
    
    return left;
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
        TokenKind.SUB
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
        case NULL: nextcase;
        case BOOL: nextcase;
        case NUMBER: nextcase;
        case STRING: nextcase;
        case IDENTIFIER: {
            return mem::new(Expr, Expr {.kind = ExprKind.LITERAL, .literal = token});
        }
        default: {
            parser.panic(ParserErr.INVALID_TOKEN);
            return mem::new(Expr, Expr {});
        }
    }
}