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
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"

	"gopkg.in/irc.v4"
)

func newRes(m *irc.Message, reply string) *irc.Message {
	return &irc.Message{
		Tags:    nil,
		Prefix:  nil,
		Command: "PRIVMSG",
		Params: []string{
			m.Params[0],
			reply,
		},
	}
}

func newDirectNotice(m *irc.Message, reply string) *irc.Message {
	return &irc.Message{
		Tags:    nil,
		Prefix:  nil,
		Command: "NOTICE",
		Params: []string{
			m.Name,
			reply,
		},
	}
}

func writeUknkErr(c *irc.Client, m *irc.Message, err error) {
	c.WriteMessage(newRes(m, fmt.Sprintf("Unknown error with Moose2 API: %v", err)))
}

func logSendFailure(c *irc.Client, m *irc.Message, format string, err error) {

	log.Printf(format, err)
	c.WriteMessage(newRes(m, fmt.Sprintf(format, err)))
}

func discardAndCloseBody(resp *http.Response) {
	io.Copy(io.Discard, io.LimitReader(resp.Body, 1048576 /* 1MiB */))
	resp.Body.Close()
}

func decodeBody(r io.Reader, v any) error {
	decoder := json.NewDecoder(r)
	return decoder.Decode(v)
}
