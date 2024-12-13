# echo

The `echo` command formats its argument and returns it as a message response. If the parameter is a string it will be
echoed as a message unformatted. If it's an array, each value will be echoed as its own response. Other JSON objects
will be formatted and displayed.

## Examples

```
> echo "Hello"
   Hello
```

```
> echo [ "One", "Two" ]
   One
   Two
```

```
> echo { "key": "value" }
   {
     "key": "value"
   }
```
