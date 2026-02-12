/****************************************************************************
**
** Copyright (C) 2015 The Qt Company Ltd.
** Contact: http://www.qt.io/licensing/
**
** This file is part of the QtVersit module of the Qt Toolkit.
**
** $QT_BEGIN_LICENSE:LGPL21$
** Commercial License Usage
** Licensees holding valid commercial Qt licenses may use this file in
** accordance with the commercial license agreement provided with the
** Software or, alternatively, in accordance with the terms contained in
** a written agreement between you and The Qt Company. For licensing terms
** and conditions see http://www.qt.io/terms-conditions. For further
** information use the contact form at http://www.qt.io/contact-us.
**
** GNU Lesser General Public License Usage
** Alternatively, this file may be used under the terms of the GNU Lesser
** General Public License version 2.1 or version 3 as published by the Free
** Software Foundation and appearing in the file LICENSE.LGPLv21 and
** LICENSE.LGPLv3 included in the packaging of this file. Please review the
** following information to ensure the GNU Lesser General Public License
** requirements will be met: https://www.gnu.org/licenses/lgpl.html and
** http://www.gnu.org/licenses/old-licenses/lgpl-2.1.html.
**
** As a special exception, The Qt Company gives you certain additional
** rights. These rights are described in The Qt Company LGPL Exception
** version 1.1, included in the file LGPL_EXCEPTION.txt in this package.
**
** $QT_END_LICENSE$
**
****************************************************************************/

#ifndef QVERSITDEFS_P_H
#define QVERSITDEFS_P_H

//
//  W A R N I N G
//  -------------
//
// This file is not part of the Qt API.  It exists purely as an
// implementation detail.  This header file may change from version to
// version without notice, or even be removed.
//
// We mean it.
//

#include <QtContacts/qcontactdetail.h>

#include <QtVersit/qversitglobal.h>

QTCONTACTS_USE_NAMESPACE

QT_BEGIN_NAMESPACE_VERSIT

// Mapping between a string in versit specifications and Qt contacts
struct VersitContextMapping {
    const char* versitString;
    const int contactContext;
};

// Mapping between a string in versit specifications and Qt contacts
struct VersitSubTypeMapping {
    const char* versitString;
    const QContactDetail::DetailType detailType;
    const int contactSubType;
};

struct VersitFileExtensionMapping {
    const char *mimeType;
    const char *extension;
};

// Mapping between a string in versit specifications and Qt contact details
struct VersitContactDetailMapping {
    const char* versitPropertyName;
    const QContactDetail::DetailType detailType;
    const int detailField;
};

//! [File extension mappings]
// Mappings from mime types to file extensions
const VersitFileExtensionMapping versitFileExtensionMappings[] = {
    {"application/octet-stream", "obj"},
    {"audio/x-pn-realaudio", "ra"},
    {"application/xml", "wsdl"},
    {"application/octet-stream", "dll"},
    {"image/x-cmu-raster", "ras"},
    {"application/x-pn-realaudio", "ram"},
    {"application/x-bcpio", "bcpio"},
    {"application/x-sh", "sh"},
    {"video/mpeg", "m1v"},
    {"image/x-xwindowdump", "xwd"},
    {"video/x-msvideo", "avi"},
    {"image/x-ms-bmp", "bmp"},
    {"application/x-shar", "shar"},
    {"application/x-javascript", "js"},
    {"application/x-wais-source", "src"},
    {"application/x-dvi", "dvi"},
    {"audio/x-aiff", "aif"},
    {"text/plain", "ksh"},
    {"application/msword", "dot"},
    {"message/rfc822", "mht"},
    {"application/x-pkcs12", "p12"},
    {"text/css", "css"},
    {"application/x-csh", "csh"},
    {"application/vnd.ms-powerpoint", "pwz"},
    {"application/pdf", "pdf"},
    {"application/x-netcdf", "cdf"},
    {"text/plain", "pl"},
    {"text/plain", "c"},
    {"image/jpeg", "jpe"},
    {"image/jpeg", "jpg"},
    {"text/x-python", "py"},
    {"text/xml", "xml"},
    {"image/jpeg", "jpeg"},
    {"application/postscript", "ps"},
    {"application/x-gtar", "gtar"},
    {"image/x-xpixmap", "xpm"},
    {"application/x-hdf", "hdf"},
    {"message/rfc822", "nws"},
    {"text/tab-separated-values", "tsv"},
    {"application/xml", "xpdl"},
    {"application/pkcs7-mime", "p7c"},
    {"application/postscript", "eps"},
    {"image/ief", "ief"},
    {"application/octet-stream", "so"},
    {"application/vnd.ms-excel", "xlb"},
    {"image/x-portable-bitmap", "pbm"},
    {"application/x-texinfo", "texinfo"},
    {"application/vnd.ms-excel", "xls"},
    {"application/x-tex", "tex"},
    {"text/richtext", "rtx"},
    {"text/html", "html"},
    {"audio/x-aiff", "aiff"},
    {"audio/x-aiff", "aifc"},
    {"application/octet-stream", "exe"},
    {"text/x-sgml", "sgm"},
    {"image/tiff", "tif"},
    {"video/mpeg", "mpeg"},
    {"application/x-ustar", "ustar"},
    {"image/gif", "gif"},
    {"application/vnd.ms-powerpoint", "ppt"},
    {"application/vnd.ms-powerpoint", "pps"},
    {"text/x-sgml", "sgml"},
    {"image/x-portable-pixmap", "ppm"},
    {"application/x-latex", "latex"},
    {"text/plain", "bat"},
    {"video/quicktime", "mov"},
    {"application/vnd.ms-powerpoint", "ppa"},
    {"application/x-troff", "tr"},
    {"application/xml", "rdf"},
    {"application/xml", "xsl"},
    {"message/rfc822", "eml"},
    {"application/x-netcdf", "nc"},
    {"application/x-sv4cpio", "sv4cpio"},
    {"application/octet-stream", "bin"},
    {"text/plain", "h"},
    {"application/x-tcl", "tcl"},
    {"application/msword", "wiz"},
    {"application/octet-stream", "o"},
    {"application/octet-stream", "a"},
    {"application/postscript", "ai"},
    {"audio/x-wav", "wav"},
    {"text/x-vcard", "vcf"},
    {"image/x-xbitmap", "xbm"},
    {"text/plain", "txt"},
    {"audio/basic", "au"},
    {"application/x-troff", "t"},
    {"image/tiff", "tiff"},
    {"application/x-texinfo", "texi"},
    {"application/oda", "oda"},
    {"application/x-troff-ms", "ms"},
    {"image/x-rgb", "rgb"},
    {"application/x-troff-me", "me"},
    {"application/x-sv4crc", "sv4crc"},
    {"video/quicktime", "qt"},
    {"video/mpeg", "mpa"},
    {"video/mpeg", "mpg"},
    {"video/mpeg", "mpe"},
    {"application/msword", "doc"},
    {"image/x-portable-graymap", "pgm"},
    {"application/vnd.ms-powerpoint", "pot"},
    {"application/x-mif", "mif"},
    {"application/x-troff", "roff"},
    {"text/html", "htm"},
    {"application/x-troff-man", "man"},
    {"text/x-setext", "etx"},
    {"application/zip", "zip"},
    {"video/x-sgi-movie", "movie"},
    {"application/x-python-code", "pyc"},
    {"image/png", "png"},
    {"application/x-pkcs12", "pfx"},
    {"message/rfc822", "mhtml"},
    {"application/x-tar", "tar"},
    {"image/x-portable-anymap", "pnm"},
    {"application/x-python-code", "pyo"},
    {"audio/basic", "snd"},
    {"application/x-cpio", "cpio"},
    {"application/x-shockwave-flash", "swf"},
    {"audio/mpeg", "mp3"},
    {"audio/mpeg", "mp2"}
};
//! [File extension mappings]

QT_END_NAMESPACE_VERSIT

#endif // QVERSITDEFS_P_H
