module lexer;
import std::io;
import std::collections::list;

def TokenList = List(<Token*>);
def StrList = List(<char[]>);

enum TokenKind {
	INVALID,
	NUMBER, IDENTIFIER,
	STRING, BOOL, NULL,
	VAR_KEY, IF_KEY, ELSE_KEY, WHILE_KEY, FOR_KEY,
	ADD, SUB, MUL, DIV,
	AND, OR, NOT,
	EQUAL, EQUALEQUAL, BANG, BANGEQUAL,
	LESS, LESSEQUAL, GREATER, GREATEREQUAL,
	COMMA, LEFTPAREN, RIGHTPAREN, LEFTBRACKET, RIGHTBRACKET, LEFTBRACE, RIGHTBRACE,
}
fn bool TokenKind.match_any(TokenKind self, TokenKind[] kinds) {
	foreach(kind : kinds) {
		if (self == kind) { return true; }
	}

	return false;
}

const char[][*] KEYWORDS = {
	"true", "false", "and", "or", "not", "null", "var", "if", "else", "while", "for"
};

struct Location {
	usz index, line, col;
}
fn void Location.next(Location* self) {
	self.index += 1;
	self.col += 1;
}
fn void Location.next_line(Location* self) {
	self.index += 1;
	self.line += 1;
	self.col = 0;
}

struct Token {
	char[] lexeme;
	TokenKind kind;
	Location loc;
}
fn void Token.debug(Token self) {
	io::printfn("Token:\t'%10s',\tKind:\t%10s", self.lexeme, self.kind);
}


fault LexerErr {
	INVALID_CHARACTER,
	INVALID_NUMBER
}

struct Lexer {
	char[] text;
	Location loc;
	TokenList tokens;
	bool panicked;
}
fn bool Lexer.is_at_end(Lexer self) {
	return self.loc.index >= self.text.len;
}
fn char Lexer.current(Lexer self) {
	if (self.is_at_end()) return '\0';
	return self.text[self.loc.index];
}
fn void Lexer.eat(Lexer* self) {
	if (self.is_at_end()) return;
	if (self.current() == '\n') {
		self.loc.next_line();
	}
	else { self.loc.next(); }
}
fn bool Lexer.eat_if(Lexer* self, char target) {
	bool matched = self.current() == target;
	if (matched) self.eat();
	return matched;
}
fn void Lexer.panic(Lexer* self, LexerErr err) {
	String msg; 
	if (self.is_at_end()) { msg = string::new_format("Lexer error %s at end", err); }
	else { 
		msg = string::new_format("Lexer error %s at line %d, col %d",
														err, 
														self.loc.line,
														self.loc.col);
	}

	io::eprintn(msg);
	self.loc.index = self.text.len;
	self.panicked = true;
}

fn TokenList scan(char[] input) {
	Lexer lexer =  { .text = input, .loc = Location {.line = 1} };

	while ( !lexer.is_at_end() ) {
		char c = lexer.current();

		if (c.is_space()) { 
			lexer.eat();
			continue; 
		}

		if (c == '/' && lexer.eat_if('/')) {
			// FIXME: not sure if this will work with files
			while (!lexer.is_at_end()) { lexer.eat(); }
			continue;
		}

		Location start = lexer.loc;
		lexer.eat();
		TokenKind kind = TokenKind.INVALID;

		switch (c) {
			case '(': kind = TokenKind.LEFTPAREN;
			case ')': kind = TokenKind.RIGHTPAREN;
			case '{': kind = TokenKind.LEFTBRACE;
			case '}': kind = TokenKind.RIGHTBRACE;
			case '+': kind = TokenKind.ADD;
			case '-': kind = TokenKind.SUB;
			case '*': kind = TokenKind.MUL;
			case '/': kind = TokenKind.DIV;

			case '=': {
				if (lexer.eat_if('=')) { kind = TokenKind.EQUALEQUAL; }
				else { kind = TokenKind.EQUAL; }
			}
			case '!': {
				if (lexer.eat_if('=')) { kind = TokenKind.BANGEQUAL; }
				else { kind = TokenKind.BANG; }
			}
			case '>': {
				if (lexer.eat_if('=')) { kind = TokenKind.GREATEREQUAL; }
				else { kind = TokenKind.GREATER; }
			}
			case '<': {
				if (lexer.eat_if('=')) { kind = TokenKind.LESSEQUAL; }
				else { kind = TokenKind.LESSEQUAL; }
			}

			case '"': {
				// TODO: string lexing here
				unreachable();
			}

			default: {
				if (c.is_digit()) {
					scan_number(&lexer);
					kind = TokenKind.NUMBER;
				} else if (c.is_alpha()) {
					scan_identifier(&lexer);					
					
					TokenKind maybe_keyword = scan_keyword(input[start.index..lexer.loc.index-1]);

					if (maybe_keyword != TokenKind.INVALID) {
						kind = maybe_keyword;
					} else { 
						kind = TokenKind.IDENTIFIER;
					}
				} else {
					lexer.panic(LexerErr.INVALID_CHARACTER);
				}
			}
		}

		Token* token = mem::new(Token, Token {
			.lexeme = input[start.index..lexer.loc.index-1],
			.loc = start,
			.kind = kind,
		});

		lexer.tokens.push(token);
	}

	if (lexer.panicked) return TokenList {};
	return lexer.tokens;
}

fn void scan_number(Lexer* lexer) {
	while (lexer.current().is_digit()) { lexer.eat(); }
	if (!lexer.eat_if('.')) { return; }
	if (!lexer.current().is_digit()) { lexer.panic(LexerErr.INVALID_NUMBER); }
	while (lexer.current().is_digit()) { lexer.eat(); }
}

fn void scan_string(Lexer* lexer) {
	// TOOD: implement strings
	unreachable();
}

fn void scan_identifier(Lexer* lexer) {
	while (lexer.current().is_alnum()) { lexer.eat(); }
}

fn TokenKind scan_keyword(char[] maybe_keyword) {
	bool is_keyword = false;
	foreach (keyword : KEYWORDS) {
		if (equals(maybe_keyword, keyword)) {
			is_keyword = true;
			break;
		}
	}
				
	if (!is_keyword) return TokenKind.INVALID;

	switch (maybe_keyword) {
		case "true": nextcase;
		case "false": return TokenKind.BOOL;

		case "and": return TokenKind.AND;
		case "or": return TokenKind.OR;
		case "not": return TokenKind.NOT;

		case "null": return TokenKind.NULL;

		case "var": return TokenKind.VAR_KEY;
		case "if": return TokenKind.IF_KEY;
		case "else": return TokenKind.ELSE_KEY;
		case "while": return TokenKind.WHILE_KEY;
		case "for": return TokenKind.FOR_KEY;

		default: unreachable();
	}
}