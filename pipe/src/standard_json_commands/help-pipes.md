# Pipes

A pipe is created using the `|` operator and is used for connecting the output of one command
to the input of another.

The target (right-hand-side) of a pipe must be a command that receives data from an IO stream:
pipes are not used to substitute the parameters of a command. It's an error to use a right-hand
side that does not provide an IO stream.

The behaviour of a pipe depends on the output of the left-hand side of the pipe:

| Left-hand side output | Result |
| ------ | ------ |
| IO stream | Pipe connects the output of the IO stream from the left-hand side to the input of the IO stream from the right-hand side of the pipe. The result is an IO stream that sends its input to the left-hand side and produces output from the right-hand side. |
| Background stream | If there's no output stream, a background stream can be sent as the input to the right-hand side of the pipe. The result will be an IO stream that accepts no input and produces the output from the right-hand side of the pipe. |
| JSON | If there's neither an IO stream nor a background stream, the JSON output of the left-hand side command will be used as input to the IO stream used by the right-hand side |

