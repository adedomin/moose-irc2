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
	"errors"
	"fmt"
	"log"
	"net/http"
	"net/url"

	"github.com/adedomin/moose-irc2/config"
)

type resolveBody struct {
	Status string
	Msg    string
}

var noSuchMoose = errors.New("No such moose.")
var malformedLine = errors.New("Line too long.")

func resolveLatestRandom(moose string) (string, error) {
	var resp *http.Response
	var err error
	resp, err = http.Get(fmt.Sprintf("%s/api-helper/resolve/%s", config.C.MooseUrl, url.PathEscape(moose)))
	if err != nil {
		log.Printf("Error: failed to talk to moose URL: %s", err)
		return "", err
	}
	defer discardAndCloseBody(resp)

	if resp.StatusCode == 404 {
		return "", noSuchMoose
	}

	var body resolveBody
	err = decodeBody(resp.Body, &body)
	if err != nil {
		log.Printf("Error: moose resolver; invalid response body: %s", err)
		return "", err
	}
	// already encoded on the server.
	return body.Msg, nil
}
