(ns hatchet.core
  (:require [instaparse.core :as insta]
            [clojure.string  :as str]
            [clojure.walk    :refer [walk postwalk]]
            [clojure.java.io :as io]))


(def idl-parser (insta/parser (io/file "./components/hatchet/resources/grammars/idl.txt") {:as {:auto-whitespace :comma}}))

(-> "./behave-lib/include/idl/behave.idl"
    (io/file)
    (slurp)
    (idl-parser))

(def sections (-> "./behave-lib/include/idl/behave.idl"
                  (io/file)
                  (slurp)
                  (str/split #"\};")))

(def sections (map #(str % "};") sections))

(map idl-parser sections)


(def cpp-test "
struct AreaUnits
{
    enum AreaUnitsEnum
    {
        SquareFeet, // base area unit
        Acres,
        Hectares,
        SquareMeters,
        SquareMiles,
        SquareKilometers
    };

    static double toBaseUnits(double value, AreaUnitsEnum units);
    static double fromBaseUnits(double value, AreaUnitsEnum units);
};")

(insta/parser "

(* Keywords *)
Alignas ::= 'alignas'
Alignof ::= 'alignof'
Asm ::= 'asm'
Auto ::= 'auto'
Bool ::= 'bool'
Break ::= 'break'
Case ::= 'case'
Catch ::= 'catch'
Char ::= 'char'
Char16 ::= 'char16_t'
Char32 ::= 'char32_t'
Class ::= 'class'
Const ::= 'const'
Constexpr ::= 'constexpr'
Const_cast ::= 'const_cast'
Continue ::= 'continue'
Decltype ::= 'decltype'
Default ::= 'default'
Delete ::= 'delete'
Do ::= 'do'
Double ::= 'double'
Dynamic_cast ::= 'dynamic_cast'
Else ::= 'else'
Enum ::= 'enum'
Explicit ::= 'explicit'
Export ::= 'export'
Extern ::= 'extern'
False_ ::= 'false'
Final ::= 'final'
Float ::= 'float'
For ::= 'for'
Friend ::= 'friend'
Goto ::= 'goto'
If ::= 'if'
Inline ::= 'inline'
Int ::= 'int'
Long ::= 'long'
Mutable ::= 'mutable'
Namespace ::= 'namespace'
New ::= 'new'
Noexcept ::= 'noexcept'
Nullptr ::= 'nullptr'
Operator ::= 'operator'
Override ::= 'override'
Private ::= 'private'
Protected ::= 'protected'
Public ::= 'public'
Register ::= 'register'
Reinterpret_cast ::= 'reinterpret_cast'
Return ::= 'return'
Short ::= 'short'
Signed ::= 'signed'
Sizeof ::= 'sizeof'
Static ::= 'static'
Static_assert ::= 'static_assert'
Static_cast ::= 'static_cast'
Struct ::= 'struct'
Switch ::= 'switch'
Template ::= 'template'
This ::= 'this'
Thread_local ::= 'thread_local'
Throw ::= 'throw'
True_ ::= 'true'
Try ::= 'try'
Typedef ::= 'typedef'
Typeid_ ::= 'typeid'
Typename_ ::= 'typename'
Union ::= 'union'
Unsigned ::= 'unsigned'
Using ::= 'using'
Virtual ::= 'virtual'
Void ::= 'void'
Volatile ::= 'volatile'
Wchar ::= 'wchar_t'
While ::= 'while'
LeftParen ::= '('
RightParen ::= ')'
LeftBracket ::= '['
RightBracket ::= ']'
LeftBrace ::= '{'
RightBrace ::= '}'
Plus ::= '+'
Minus ::= '-'
Star ::= '*'
Div ::= '/'
Mod ::= '%'
Caret ::= '^'
And ::= '&'
Or ::= '|'
Tilde ::= '~'
Not ::= '!' | 'not'
Assign ::= '='
Less ::= '<'
Greater ::= '>'
PlusAssign ::= '+='
MinusAssign ::= '-='
StarAssign ::= '*='
DivAssign ::= '/='
ModAssign ::= '%='
XorAssign ::= '^='
AndAssign ::= '&='
OrAssign ::= '|='
LeftShiftAssign ::= '<<='
RightShiftAssign ::= '>>='
Equal ::= '=='
NotEqual ::= '!='
LessEqual ::= '<='
GreaterEqual ::= '>='
AndAnd ::= '&&' | 'and'
OrOr ::= '||' | 'or'
PlusPlus ::= '++'
MinusMinus ::= '--'
Comma ::= ','
ArrowStar ::= '->*'
Arrow ::= '->'
Question ::= '?'
Colon ::= ':'
Doublecolon ::= '::'
Semi ::= '';
Dot ::= '.'
DotStar ::= '.*'
Ellipsis ::= '...'

IntegerLiteral ::= ( DecimalLiteral | OctalLiteral | HexadecimalLiteral | BinaryLiteral ) Integersuffix?
CharacterLiteral ::= ( 'u' | 'U' | 'L' )? \"'\" Cchar+ \"'\"
FloatingLiteral ::= ( Fractionalconstant Exponentpart? | Digitsequence Exponentpart ) Floatingsuffix?
StringLiteral ::= '\\\"' Schar* '\\\"'
Hexquad  ::= HEXADECIMALDIGIT HEXADECIMALDIGIT HEXADECIMALDIGIT HEXADECIMALDIGIT
Universalcharactername ::= ( '\\\\u' | '\\\\U' Hexquad ) Hexquad
Identifier ::= #'[a-zA-Z_][a-zA-Z0-9_]*'
Identifiernondigit ::= NONDIGIT | Universalcharactername
NONDIGIT ::= #'[a-zA-Z_]'
DIGIT    ::= #'[0-9]'
DecimalLiteral ::= #'[1-9][0-9]*'
OctalLiteral ::= '0' ( \"'\"? #'[0-7]' )*
HexadecimalLiteral ::= ( '0x' | '0X' ) HEXADECIMALDIGIT ( \"'\"? HEXADECIMALDIGIT )*
BinaryLiteral ::= ( '0b' | '0B' ) BINARYDIGIT ( \"'\"? BINARYDIGIT )*
NONZERODIGIT ::= #'[1-9]'
HEXADECIMALDIGIT ::= #'[0-9a-fA-F]'
BINARYDIGIT ::= #'[0-1]'
Escapesequence ::= Simpleescapesequence | Octalescapesequence | Hexadecimalescapesequence
Fractionalconstant ::= Digitsequence? '.' Digitsequence | Digitsequence '.'
Exponentpart ::= ( 'e' | 'E' ) SIGN? Digitsequence
SIGN     ::= #'[\\+\\-]'
Digitsequence ::= DIGIT ( \"'\"? DIGIT )*
Floatingsuffix ::= #'[flFL]'
Encodingprefix ::= 'u8' | 'u' | 'U' | 'L'
Integersuffix ::= Unsignedsuffix ( Longsuffix | Longlongsuffix )? | ( Longsuffix | Longlongsuffix ) Unsignedsuffix?
Unsignedsuffix ::= #'[uU]'
Longsuffix ::= #'[lL]'
Longlongsuffix ::= 'll' | 'LL'
Simpleescapesequence ::= #'[\n\r\f\b]'
                       | '\\''
                       | '\"'
                       | '\\\\'
Semicolon ::= ';' 
Octalescapesequence ::= '\\\\' #'[0-7]' ( #'[0-7]' #'[0-7]'? )?
Hexadecimalescapesequence ::= '\\\\x' HEXADECIMALDIGIT+
Cchar    ::= #'[^\\'\\r\\n]+'
           | Escapesequence
           | Universalcharactername
Schar    ::= #'[^\\\"\\r\\n]+'
Whitespace ::= ( #'[\\s\n\r]'+ | Newline+ )
S        ::= #'[\\s\n\r]+'
Newline  ::= ('\\r' '\\n'? | '\\n')
LineComment ::= '//' #'[^\\r\\n]'

Whitespace ::= #'[ \\n]'+
Newline  ::= ('\\r' '\\n'? | '\\n')
BlockComment = '\\\\/\\\\*' #'.*' '\\\\*\\\\/'
LineComment ::= '//' #'[^\\n\\s]'*

")
(def idl-parser (insta/parser (io/file "./components/hatchet/resources/grammars/cpp.txt"))


(def idl-parser
  (insta/parser "
definitions               ::= ( definition ';' S? )*
definition                ::= interfaceDefinition | enumDefinition
enumDefinition            ::= 'enum' S? Identifier S? '{' S? enumValues? '}'
enumValues                ::= StringLiteral S? ( ',' S? StringLiteral )*
interfaceDefinition       ::= extendedAttributes? 'interface' S? interfaceName S? '{' S? memberDefinitionSeq? S? '}' 
interfaceName             ::= Identifier
memberDefinitionSeq       ::= memberDefinition*
memberDefinition          ::= extendedAttributes? S? attributes? S? theType S? Identifier S? argumentList? ';' 
argumentList              ::= '(' arguments* ')'
arguments                 ::= argument ( ',' S? argument )*
argument                  ::= extendedAttributes? attribute? S? theType S? Identifier S? theDefault?
theDefault                ::= ( '=' S? defaultValue )?
defaultValue              ::= IntegerLiteral
                            | StringLiteral
                            | '[' ']'
                            | '{' '}'
attributes                ::= attribute+
attribute                 ::= 'attribute'
                            | 'static'
                            | 'optional'
                            | 'readonly'
theType                   ::= arrayType | singleType
singleType                ::= distinguishableType | 'any'
arrayType                 ::= distinguishableType '[' ']'
distinguishableType       ::= primitiveType | Identifier
primitiveType             ::= unsignedIntegerType
                            | unrestrictedFloatType
                            | 'boolean'
                            | 'bigint'
                            | 'byte'
                            | 'DOMString'
                            | 'octet'
                            | 'void'
                            | 'VoidPtr'
unsignedIntegerType       ::= 'unsigned'? integerType
integerType               ::= 'short' | 'long' 'long'?
unrestrictedFloatType     ::= 'unrestricted'? floatType
floatType                 ::= 'float' | 'double'
extendedAttributes        ::= '[' extendedAttribute ( ',' extendedAttribute )* ']'
extendedAttribute         ::= extendedAttributeNoArgs | extendedAttributeIdent
extendedAttributeNoArgs   ::= 'Const'
                            | 'BoundsChecked'
                            | 'Ref'
                            | 'Value'
                            | Identifier
extendedAttributeIdent    ::= ( 'JSImplementation' | 'Operator' | 'Prefix' ) '=' StringLiteral
IntegerLiteral            ::= ( DecimalLiteral | OctalLiteral | HexadecimalLiteral | BinaryLiteral ) Integersuffix?
CharacterLiteral          ::= ( 'u' | 'U' | 'L' )? \"'\" Cchar+ \"'\"
FloatingLiteral           ::= ( Fractionalconstant Exponentpart? | Digitsequence Exponentpart ) Floatingsuffix?
StringLiteral             ::= '             \\\"' Schar* '\\\"'
Hexquad                   ::= HEXADECIMALDIGIT HEXADECIMALDIGIT HEXADECIMALDIGIT HEXADECIMALDIGIT
Universalcharactername    ::= ( '  \\\\u' | '\\\\U' Hexquad ) Hexquad
Identifier                ::= #'[a-zA-Z_][a-zA-Z0-9_]*'
Identifiernondigit        ::= NONDIGIT | Universalcharactername
NONDIGIT                  ::= #'[a-zA-Z_]'
DIGIT                     ::= #'[0-9]'
DecimalLiteral            ::= #'[1-9][0-9]*'
OctalLiteral              ::= '0' ( \"'\"? #'[0-7]' )*
HexadecimalLiteral        ::= ( '0x' | '0X' ) HEXADECIMALDIGIT ( \"'\"? HEXADECIMALDIGIT )*
BinaryLiteral             ::= ( '0b' | '0B' ) BINARYDIGIT ( \"'\"? BINARYDIGIT )*
NONZERODIGIT              ::= #'[1-9]'
HEXADECIMALDIGIT          ::= #'[0-9a-fA-F]'
BINARYDIGIT               ::= #'[0-1]'
Escapesequence            ::= Simpleescapesequence | Octalescapesequence | Hexadecimalescapesequence
Fractionalconstant        ::= Digitsequence? '.' Digitsequence | Digitsequence '.'
Exponentpart              ::= ( 'e' | 'E' ) SIGN? Digitsequence
SIGN                      ::= #'[                \\+\\-]'
Digitsequence             ::= DIGIT ( \"'\"? DIGIT )*
Floatingsuffix            ::= #'[flFL]'
Encodingprefix            ::= 'u8' | 'u' | 'U' | 'L'
Integersuffix             ::= Unsignedsuffix ( Longsuffix | Longlongsuffix )? | ( Longsuffix | Longlongsuffix ) Unsignedsuffix?
Unsignedsuffix            ::= #'[uU]'
Longsuffix                ::= #'[lL]'
Longlongsuffix            ::= 'll' | 'LL'
Simpleescapesequence      ::= #'[\n\r\f\b]'
                            | '      \\''
                            | '\"'
                            | '      \\\\'
Semicolon                 ::= ';' 
Octalescapesequence       ::= '       \\\\' #'[0-7]' ( #'[0-7]' #'[0-7]'? )?
Hexadecimalescapesequence ::= ' \\\\x' HEXADECIMALDIGIT+
Cchar                     ::= #'[^               \\'\\r\\n]+'
                          | Escapesequence
                          | Universalcharactername
Schar                     ::= #'[^               \\\"\\r\\n]+'
Whitespace                ::= ( #'[            \\s\n\r]'+ | Newline+ )
S                         ::= #'[                \\s\n\r]+'
Newline                   ::= ('                 \\r' '\\n'? | '\\n')
LineComment               ::= '//' #'[^       \\r\\n]'
BlockComment              ::= '\\\\/\\\\*' #'.*' '\\\\*\\\\/'
_                         ::= Whitespace
                            | Newline
                            | BlockComment
                            | LineComment "))
