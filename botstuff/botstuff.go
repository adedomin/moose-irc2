/*
 *  BSD 2-Clause License
 *
 *  Copyright (c) 2024, Anthony DeDominic
 *
 *  Redistribution and use in source and binary forms, with or without
 *  modification, are permitted provided that the following conditions are met:
 *
 *  1. Redistributions of source code must retain the above copyright notice, this
 *     list of conditions and the following disclaimer.
 *
 *  2. Redistributions in binary form must reproduce the above copyright notice,
 *     this list of conditions and the following disclaimer in the documentation
 *     and/or other materials provided with the distribution.
 *
 *  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 *  AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 *  IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 *  DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 *  FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 *  DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 *  SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 *  CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 *  OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 *  OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
package botstuff

import (
	"fmt"
	"strings"

	"github.com/adedomin/moose-irc2/config"
	"gopkg.in/irc.v4"
)

func IrcHandler(c *irc.Client, m *irc.Message) {
	switch m.Command {
	case irc.RPL_WELCOME:
		c.Write(config.SplitChannelList(config.C.Channels))
	case "INVITE":
		handleInvite(c, m)
	case "PART":
		if len(m.Params) < 1 {
			return
		}
		channelName := m.Params[0]
		if c.CurrentNick() == m.Name {
			handlePartKick(channelName, "PARTed")
		}
	case "KICK":
		if len(m.Params) < 3 {
			return
		}
		channelName := m.Params[0]
		target := m.Params[1]
		reason := m.Params[2]
		if target == c.CurrentNick() {
			handlePartKick(channelName, reason)
		}
	case "PRIVMSG":
		if len(m.Params) < 2 {
			return
		}
		// Trim leading and trailing spaces to not trip up our
		// plugins.
		m.Params[1] = strings.TrimSpace(m.Params[1])
		comm := parseMooseArgs(m.Params[1])
		switch comm.cmd {
		case mIrc, mImg:
			handleApiCommand(comm, c, m)
		case mSearch:
			handleSearch(comm.moose, c, m)
		case mBots:
			c.WriteMessage(newRes(m, fmt.Sprintf("Moose :: Make moose @ %s :: See .moose --help for usage", config.C.MooseUrl)))
		case mHelp:
			c.WriteMessage(newRes(m, "usage: ^[.!]?moose(?:img|search|me)? [--latest|--oldest|--random|--search|--image|--] moosename"))
		}
	}
}
