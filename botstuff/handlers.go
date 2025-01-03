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
	"bufio"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"strings"
	"sync/atomic"
	"time"

	"github.com/adedomin/moose-irc2/config"
	"gopkg.in/irc.v4"
)

var lastMoose atomic.Int64

func handleApiCommand(comm command, c *irc.Client, m *irc.Message) {
	oldTime := lastMoose.Load()
casloop:
	for {
		now := time.Now().Unix()
		if now-oldTime > 10 {
			if !lastMoose.CompareAndSwap(oldTime, now) {
				oldTime = lastMoose.Load()
			} else {
				break casloop
			}
		} else {
			c.WriteMessage(newDirectNotice(m, "Please wait"))
			return
		}
	}
	mooseName, err := resolveLatestRandom(comm.moose)
	if err != nil {
		if errors.Is(err, noSuchMoose) {
			c.WriteMessage(newRes(m, fmt.Sprintf("No such moose: %s", comm.moose)))
		} else {
			logSendFailure(c, m, "ERROR: Moose Resolution Error: %s", err)
		}
		return
	}
	if comm.cmd == mIrc {
		resp, err := http.Get(fmt.Sprintf("%s/irc/%s", config.C.MooseUrl, mooseName))
		if err != nil {
			logSendFailure(c, m, "ERROR: Failed to talk to moose service: %s", err)
			return
		}
		defer discardAndCloseBody(resp)

		if resp.StatusCode != 200 {
			c.WriteMessage(newRes(m, fmt.Sprintf("Unexpected Status getting moose: %s", resp.Status)))
			return
		}

		bufread := bufio.NewReader(resp.Body)
		for {
			line, prefix, err := bufread.ReadLine()
			if err == io.EOF {
				break
			} else if err != nil {
				logSendFailure(c, m, "ERROR: Failed to read from moose response: %s", err)
				return
			} else if prefix {
				logSendFailure(c, m, "ERROR: Malformed moose line: %s", malformedLine)
				return
			}
			c.WriteMessage(newRes(m, string(line)))
		}
	}
	c.WriteMessage(newRes(m, fmt.Sprintf("%s/img/%s", config.C.MooseUrl, mooseName)))
}

type moose struct {
	Name string
	// rest omitted
}

type searchInnerBody struct {
	Page  int
	Moose moose
}

type searchBody struct {
	Pages  int
	Result []searchInnerBody
}

const maxResultSet = 12

func handleSearch(query string, c *irc.Client, m *irc.Message) {
	if config.C.DisableSearch {
		c.WriteMessage(newRes(m, "Search is disabled because of network spam filter."))
		return
	}

	querySafe := url.QueryEscape(query)
	resp, err := http.Get(fmt.Sprintf("%s/search?q=%s&p=0", config.C.MooseUrl, querySafe))
	if err != nil {
		logSendFailure(c, m, "ERROR: Failed to talk to moose service: %s", err)
		return
	}
	defer discardAndCloseBody(resp)

	var body searchBody
	err = decodeBody(resp.Body, &body)
	if err != nil {
		logSendFailure(c, m, "ERROR: Moose search result was malformed: %s", err)
		return
	}

	if len(body.Result) == 0 {
		c.WriteMessage(newRes(m, fmt.Sprintf("No moose found: %s", query)))
		return
	}

	s := make([]string, 0, maxResultSet)
	for _, val := range body.Result {
		s = append(s, fmt.Sprintf("\x02%s\x02 p.%d", val.Moose.Name, val.Page))
	}

	c.WriteMessage(newRes(m, strings.Join(s, ", ")))
}

func handleInvite(c *irc.Client, m *irc.Message) {
	if config.C.InviteFile == "" {
		log.Printf("sending notice to %s", m.Name)
		c.WriteMessage(newDirectNotice(m, "Invites are disabled."))
		return
	}
	target := m.Params[0]
	// make sure this is addressed to us
	if c.CurrentNick() != target {
		return
	}
	channelName := m.Params[1]
	// check if already invited.
	if config.HasInvite(channelName) {
		return
	}
	c.WriteMessage(&irc.Message{
		Tags:    nil,
		Prefix:  nil,
		Command: "JOIN",
		Params:  []string{channelName},
	})
	log.Printf("INFO: Invited to %s", channelName)
	config.SaveNewInvite(config.AddInvite, channelName)
}

func handlePartKick(channelName, reason string) {
	if config.HasInvite(channelName) {
		config.SaveNewInvite(config.DelInvite, channelName)
	}
	log.Printf("INFO: Removed from channel %s; reason: %s", channelName, reason)
}
