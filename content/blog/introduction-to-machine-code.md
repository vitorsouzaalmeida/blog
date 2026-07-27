---
title: Introduction to Machine Code
pubDate: 2024-11-03
tags:
  - code
  - computer-science
---

### What is machine code?

Machine code is computer code.

It consisting of machine language instructions that will be used to control a CPU. And about this, the reason that some programming languages have virtual machines, is to simulate a unique CPU. It helps to run the same code on different platforms without needing to care about specific architectures.

An opcode (Operation Code) is one byte wide, has an arbitrary but unique value, and is the first byte on the instruction.

Opcodes are just a set that specifies the operation (op) to be performed. In other words, an opcode is just an instruction machine code.

Example:

```asm
MOV AL, 34h
```

The opcode here is the MOV instruction. The other parts are the operands. In this case, the operands are the register AL and the value 34 hex. However, the operands are the parameters for the instruction. If it needs clarification, it could be more helpful to think about math. Think about **3 + 6 = 9**.
The "opcode" here, the operation, is the **+** symbol, which means addition. The operands are **3** and **6**.

### Assembler, disassembler

Of course, no one writes code in machine code — at least not in modern times. However, some tools translate code from Assembly Language to Machine Code.

In assembly language, each instruction on the computer is represented by a mnemonic:

```asm
section .data
	    msg db "Result: "         ; message to display
    number db '0'                 ; single character to store the result

section .text
    global _start

_start:
    mov rax, 1                    ; Put number 1 in rax
    add rax, 2                    ; Add 2 to it

    ; Step 2: Convert result to character
    add rax, '0'                  ; Convert number to ASCII character
    mov [number], al              ; Store the character

    ; Step 3: Print "Result: "
    mov rax, 1                    ; System call for write
    mov rdi, 1                    ; File descriptor 1 is stdout
    mov rsi, msg                  ; Address of our message
    mov rdx, 8                    ; Message length
    syscall

    ; Step 4: Print the number
    mov rax, 1                    ; System call for write
    mov rdi, 1                    ; File descriptor 1 is stdout
    mov rsi, number               ; Address of our number
    mov rdx, 1                    ; Length is 1 character
    syscall

    ; Step 5: Exit program
    mov rax, 60                   ; System call for exit
    xor rdi, rdi                  ; Return code 0
    syscall
```

This code was written using an assembler/disassembler called NASM and sums **1** to **2**. This program (NASM) will translate the mnemonics into machine code. Historically, this part is exciting. If you think about how things work nowadays, we already have compilers and all the tools needed to write code that humans can easily understand. But think a second about the process of creating a new programming language.

Of course, you will need some language to code your new language. For example, I'm writing a toy language called Monkey using GoLang. If you're creating a language for production use, eventually, you'll do the bootstrapping — basically, implement your language using the language itself. Following my example of Monkey, it is like refactoring Monkey using Monkey instead of GoLang.

Now, moving some years back, someone needed to write the first assembler/disassembler directly in a binary system. The first functional computer will depend on what you understand by computer and functional and other things. But I'll follow the idea that it was the [EDSAC](https://en.wikipedia.org/wiki/EDSAC). Initially, from binary input, programs were entered using a set of 18 switches on EDSAC's control panel. Each instruction was typically 18 bits long - that's not 18 because the topmost was always unavailable due to timing programs, so only 17 bits were used. Operators would physically flip these switches to represent 1s and 0s. And, of course, they used paper tape for program storage, but the initial loader had to be entered manually.
Then, the first simple assembler. This was a tiny 31-instruction program (31 words). It was the world's first assembler. It could read paper tape and convert simple symbolic notations into machine code. The basic format was like a Letter + Number, like A 32, which means "Add the content of memory location 32".

The first bootstrap comes from this basic assembler. The new version could handle more symbols, basic arithmetic in the address, and some macros. This process was repeated. If you want to learn more about this, I recommend reading [EDSAC](https://pt.wikipedia.org/wiki/EDSAC) from Wikipedia and watching [Bootstrapping EDSAC: Initial Orders—Computerphile](https://youtu.be/nc2q4OOK6K8?si=W7rzWRtFQDXfqA2z) from Computerphile.

### Starting with bytes

Excellent. I hope I have cleared your mind about machine code at this point. We'll write some code in GoLang to write our machine code instructions.

Let's start by defining the bytecode format:

```go title="code/code.go"
package code

type Instructions []byte // byte is an alias for uint8

type Opcode byte
```

`Instructions` are a slice of bytes, and an `Opcode` is a byte. Note how it describes our past descriptions. Let's define the first opcode, which would tells the VM (or the processor, if you're compiling directly to machine code and to some especific processor architecture) to push something on the stack - we'll not build a VM at this article.

Backing to the opcode, it wouldn't be called "push", because it won't be solely about pushing things. Let's think about the expression `1 + 2`. There are three instructions, two of which tell the VM/processor to push `1` and `2` to the stack. A first instinct might tell us to implement these by defining a "push" instruction with an integer as its operand, with the idea being that the VM/processor then takes the integer operand and pushes it into the stack. For integers, it would work because I could encode them and put them directly into the bytecode, but for string literals, for example, putting those into the bytecode is also possible since it's just made of bytes. Still, it would also be a lot of bloat and would sooner or later become unwieldy.

- Variable size: A string can be any length, like "a" or "a more extensive text like this one.";
- You'd have multiple copies of the same string if it appears several times;
- Bytecode loading performance would be impacted;

Notice how it is a bad design? Here, I introduce the idea of `constants`.

### Constants

In this context, `constants` are short for "constant expressions" and refer to expressions whose value doesn't change. It is `constant` and can be determined at `compile time`. That means we don't need to run the program to know what these expressions evaluate. A compiler can find them in the code and store the value they evaluate. After that, it can _reference_ the constants in the instructions it generates instead of embedding the value directly in them. A plain integer does the job fine and can serve as an index into a data structure that holds all constants, often called a constant pool. For example:

```asm
// Instead of having bytecode like:
PUSH 987654321 // Takes up a lot of space if the number is big
PUSH "Hello World" // Takes even more space for string

// I can have:
PUSH_CONST 0 // Where 0 is just an index into the constant pool
PUSH_CONST 1 // Much more compact

Constant Pool:
[0] -> 987654321
[1] -> "Hello World"
```

- More compact bytecode (indices are smaller than full values)
- Deduplication (the same constant only needs to be stored once)
- Better memory usage
- It is easier to manage complex constants like strings

Well, that said, let's define the `OpConstant`. This opcode has one operand: the number I previously assigned to the constant. When the VM/processor executes `OpConstant`, it retrieves the constant using the operand as an index and pushes it on to the stack.

```go title="code/code.go"
// [...]

const (
	OpConstant Opcode = iota
)
```

`Iota` will generate increasing `byte` values because I don't care about the actual values the opcodes represent. They only need to be distinct from each other and fit in one byte. Now, let's define the part that says `OpConstant` has one operand.

```go title="code/code.go"
// [...]

type Definition struct {
	Name          string
	OperandWidths []int
}

var definitions = map[Opcode]*Definition{
	OpConstant: {"OpConstant", []int{2}},
}

func Lookup(op byte) (*Definition, error) {
	def, ok := definitions[Opcode(op)]
	if !ok {
		return nil, fmt.Errorf("opcode %d undefined", op)
	}
	return def, nil
}
```

The lookup helper is not needed, but it's nice for debugging:

```go
debug, _ := Lookup(instructions[0])
fmt.Println(debug)
```

Output:

```shell
go test ./code -v
=== RUN   TestMake
&{OpConstant [2]}
--- PASS: TestMake (0.00s)
PASS
ok      github.com/vit0rr/introduction-to-machine-code/code     0.179s
```

The `Definition` for an `Opcode` has two fields: `Name` and `OperandsWidths`. `Name` helps to make an `Opcode` readable and `OperandWidths` contains the number of bytes each operand takes up.

The definition for `OpConstant` says that it's only operand two bytes wide, which makes it an uint16 and limits its maximum value to `65535`; if it includes 0, the number of representable values is then `65536`.

With this, It's already possible to create the first bytecode instruction - finally.

```go title="code/code.go"
// [...]

func Make(op Opcode, operands ...int) []byte {
	def, ok := definitions[op]
	if !ok {
		return []byte{}
	}

	instructionLen := 1
	for _, w := range def.OperandWidths {
		instructionLen += w
	}

	instruction := make([]byte, instructionLen)
	instruction[0] = byte(op)

	offset := 1
	for i, o := range operands {
		width := def.OperandWidths[i]
		switch width {
		case 2:
			binary.BigEndian.PutUint16(instruction[offset:], uint16(o))
		}
		offset += width
	}

	return instruction
}
```

This function creates a bytecode instruction for the given `Opcode` and operands. It looks up the `Definition` for the `Opcode` and calculates the instruction's length. Then it creates a byte slice with the correct length and sets the first byte to the `Opcode`. The function then iterates over the operands, encoding them into the instruction according to their width.

And of course we'll write tests about all this.

```go title="code/code_test.go"
package code

import "testing"

func TestMake(t *testing.T) {
	tests := []struct {
		op       Opcode
		operands []int
		expected []byte
	}{
		{OpConstant, []int{65534}, []byte{byte(OpConstant), 255, 254}},
	}

	for _, tt := range tests {
		instructions := Make(tt.op, tt.operands...)

		if len(instructions) != len(tt.expected) {
			t.Errorf("instructions has wrong length. want=%d, got=%d",
				len(tt.expected), len(instructions))
		}

		for i, b := range tt.expected {
			if instructions[i] != tt.expected[i] {
				t.Errorf("wrong byte at pos %d. want=%d, got=%d",
					i, b, instructions[i])
			}
		}
	}
}
```

I only pass `OpConstant` and the operand `65534` to the `Make` function. Then expect to get back a `[]byte` golding three bytes. The first has to be the opcode, `OpConstant`, and the other two should be the big-endian encoding of `65534`. And that's also why use `65534` instead of `65535`:

```text
65534 in decimal = 1111 1111 1111 1110 in binary
                 = 0xFF 0xFE in hexadecimal (two bytes)
```

Big-endian means "most significant byte first". Like reading left-to-right:

- In big-endian: 65534 = [0xFF, 0xFE]
- In little-endian: 65534 = [0xFE, 0xFF]
  And `65535`, both bytes are the same ([0xFF, 0xFF]) - can't tell the order.

The expected output is 3 bytes:

```text
[OpConstant, 0xFF, 0xFE]
 ^           ^      ^
 |           |      Second byte of 65534
 |           First byte of 65534
 The instruction opcode
```

The first thing I'm doing here is to determine how long the resulting instruction will take. That allows me to allocate a byte slice with the proper length. Note that I don't use the `Lookup` function to get to the definition, which gives me a more usable function signature for Make in the tests later.

As soon as we have the final value of `instructionLen`, we allocate the instruction `[]byte` and add the Opcode as its first byte by casting it into one. Then comes the tricky part: I iterate over the defined operand widths, taking the matching element from operands and putting it in the instructions. Depending on its width, I do that by using a switch statement with a different method for each operand.

I only ensure that a two-byte operand is encoded in big-endian. After encoding the operand, I increment the offset by its width and the next iteration of the loop. Since the `OpConstant` opcode in the test case has only one operand, the loop performs only one iteration before the `Make` returns `instruction`.

And that's it! First bytecode instruction is done.

```shell
go test ./code
ok      github.com/vit0rr/introduction-to-machine-code/code     0.537s
```

### References:

- [Opcodes and Operands](https://www.teach-ict.com/as_as_computing/ocr/H447/F453/3_3_8/features/miniweb/pg4.htm)
- [Enumeration](https://en.wikipedia.org/wiki/Enumeration)
- [Opcode](https://en.wikipedia.org/wiki/Opcode)
- [Machine Code for Beginners (Usborne Computer Books)](https://archive.org/details/machine-code-for-beginners)
- [Writing a Compiler in Go (by Thorsten Ball)](https://compilerbook.com/)
- [Virtual Machine I: Stack Arithmetic (Chapter 7)](https://minnie.tuhs.org/Tecs/book/chapter07.pdf)
- [EDSAC](https://en.wikipedia.org/wiki/EDSAC)
