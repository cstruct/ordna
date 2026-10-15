# Values source

The values source loads values the app gives itself, for config paths, such as from its
command line flags. Parsing them is the app's concern.

r[values.source]
The source MUST provide the values given for their config paths, with a later value for a
path replacing an earlier one.

r[values.strings]
Values MUST be passed on as strings, to be parsed into the field's type when the config is
decoded.

r[values.layer]
The source MUST be public by default, and secret when made so, for values of secret fields.

r[values.unknown]
A value for a path that matches no field MUST be reported as an unknown path.

r[values.detail]
The detail of each value MUST say the app gave it; naming the source tells where from.
