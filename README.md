# invogen
Invoice generator, also rust learning project

Built for a personal usecase, but if you find it useful you're welcome to use it.

## Help

```

invogen

USAGE:
    invogen [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f, --file <file>    [default: client.history]

SUBCOMMANDS:
    add-client        Add a new client
    change-address    Change a client's address
    change-name       Change a client's name
    help              Prints this message or the help of the given subcommand(s)
    invoice           Create a new invoice for a client
    list-clients      List all clients
    list-invoices     List all invoices for a client
    rates             Show billing and tax rates for a client
    set-rate          Set the billing rate for a client
    set-taxes         Set the tax rate(s) for a client
```
