import sys

with open('src/player.rs', 'r', encoding='utf-8') as f:
    content = f.read()

in_string = False
in_line_comment = False
in_block_comment = False
depth = 0

i = 0
while i < len(content):
    ch = content[i]

    if not in_string and not in_block_comment:
        if ch == '/' and i+1 < len(content) and content[i+1] == '/':
            in_line_comment = True
            i += 1
            continue
    if in_line_comment:
        if ch == '\n':
            in_line_comment = False
        i += 1
        continue

    if not in_string:
        if ch == '/' and i+1 < len(content) and content[i+1] == '*':
            in_block_comment = True
            i += 1
            continue
        if in_block_comment and ch == '*' and i+1 < len(content) and content[i+1] == '/':
            in_block_comment = False
            i += 2
            continue
    if in_block_comment:
        if ch == '\n':
            pass
        i += 1
        continue

    if not in_string:
        if ch == '"':
            in_string = True
            i += 1
            continue
    if in_string:
        if ch == '\\' and i+1 < len(content):
            i += 2
            continue
        if ch == '"':
            in_string = False
            i += 1
            continue
        i += 1
        continue

    if ch == '\n':
        i += 1
        continue

    if ch == '{':
        depth += 1
    elif ch == '}':
        depth -= 1
        if depth < 0:
            line_num = content[:i].count('\n') + 1
            print(f'NEGATIVE at line {line_num}')
            sys.exit(1)

    i += 1

print(f'Final depth: {depth}')
