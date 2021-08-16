# invogen
Invoice generator, also a project for learning rust

Built for a personal use-case, but if you find it useful you're welcome to use
it.

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
    add          Add a new client or service
    help         Prints this message or the help of the given subcommand(s)
    invoice      Generate a new invoice for a client
    list         List clients, services, or invoices
    mark-paid    Record an invoice as paid
    remove       Remove a client, all history will be maintained
    set          Set properties of clients and services
    show         Show clients and invoices
```
