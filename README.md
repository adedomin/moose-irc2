# Moose2 IRC bot

Running
=======

```sh
moose-irc2 init -c /etc/moose-irc2/NETWORK_NAME.json
# now edit /etc/moose-irc2/NETWORK_NAME.json
# if you want invites, edit invite-file in the config.json
# empty strings are the default "off" value(s)
cp moose-irc2/contrib/etc/systemd/system/moose-irc2@.service /etc/systemd/system/moose-irc2@.service
systemctl daemon-reload
systemctl enable --now moose-irc2@NETWORK_NAME.service
```
