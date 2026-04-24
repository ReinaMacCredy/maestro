# Google JSON Style Guide Frozen Snapshot

Source: https://google.github.io/styleguide/jsoncstyleguide.xml
Snapshot date: 2026-04-24
Attribution: Copied from the Google Style Guides project for Maestro bundled setup use.
License: Content is Creative Commons Attribution 3.0 (https://creativecommons.org/licenses/by/3.0/); code samples are Apache 2.0 (https://www.apache.org/licenses/LICENSE-2.0).

---

# Google JSON Style Guide

Revision 0.9

<div style="margin-left: 50%; font-size: 75%;">

Each style point has a summary for which additional information is available by toggling the accompanying arrow button that looks this way: <span class="showhide_button" style="margin-left: 0; float: none">▶</span>. You may toggle all summaries with the big arrow button:

<div style=" font-size: larger; margin-left: +2em;">

<span id="show_hide_all_button" class="showhide_button" style="font-size: 180%; float: none" onclick="javascript:ShowHideAll()">▶</span> Toggle all summaries

</div>

</div>

<div class="toc">

<div class="toc_title">

Table of Contents

</div>

<table>
<colgroup>
<col style="width: 50%" />
<col style="width: 50%" />
</colgroup>
<tbody>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#General_Guidelines">General Guidelines</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Comments">Comments</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Double_Quotes">Double Quotes</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Flattened_data_vs_Structured_Hierarchy">Flattened data vs Structured Hierarchy</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Property_Name_Guidelines">Property Name Guidelines</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Property_Name_Format">Property Name Format</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Key_Names_in_JSON_Maps">Key Names in JSON Maps</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Reserved_Property_Names">Reserved Property Names</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Singular_vs_Plural_Property_Names">Singular vs Plural Property Names</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Naming_Conflicts">Naming Conflicts</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Property_Value_Guidelines">Property Value Guidelines</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Property_Value_Format">Property Value Format</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Empty/Null_Property_Values">Empty/Null Property Values</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Enum_Values">Enum Values</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Property_Value_Data_Types">Property Value Data Types</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Date_Property_Values">Date Property Values</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Time_Duration_Property_Values">Time Duration Property Values</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Latitude/Longitude_Property_Values">Latitude/Longitude Property Values</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#JSON_Structure_&amp;_Reserved_Property_Names">JSON Structure &amp; Reserved Property Names</a>
</div></td>
<td><div class="toc_stylepoint">
&#10;</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Top-Level_Reserved_Property_Names">Top-Level Reserved Property Names</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#apiVersion">apiVersion</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#context">context</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#id">id</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#method">method</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#params">params</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data">data</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error">error</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Reserved_Property_Names_in_the_data_object">Reserved Property Names in the data object</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#data.kind">data.kind</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.fields">data.fields</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.etag">data.etag</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.id">data.id</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.lang">data.lang</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.updated">data.updated</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.deleted">data.deleted</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.items">data.items</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Reserved_Property_Names_for_Paging">Reserved Property Names for Paging</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#data.currentItemCount">data.currentItemCount</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.itemsPerPage">data.itemsPerPage</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.startIndex">data.startIndex</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.totalItems">data.totalItems</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.pagingLinkTemplate">data.pagingLinkTemplate</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.pageIndex">data.pageIndex</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.totalPages">data.totalPages</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Reserved_Property_Names_for_Links">Reserved Property Names for Links</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#data.self_/_data.selfLink">data.self / data.selfLink</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.edit_/_data.editLink">data.edit / data.editLink</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.next_/_data.nextLink">data.next / data.nextLink</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#data.previous_/_data.previousLink">data.previous / data.previousLink</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Reserved_Property_Names_in_the_error_object">Reserved Property Names in the error object</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#error.code">error.code</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.message">error.message</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors">error.errors</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors%5B%5D.domain">error.errors[].domain</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors%5B%5D.reason">error.errors[].reason</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors%5B%5D.message">error.errors[].message</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors%5B%5D.location">error.errors[].location</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors%5B%5D.locationType">error.errors[].locationType</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors%5B%5D.extendedHelp">error.errors[].extendedHelp</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#error.errors%5B%5D.sendReport">error.errors[].sendReport</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Property_Ordering">Property Ordering</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Kind_Property">Kind Property</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Items_Property">Items Property</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Property_Ordering_Example">Property Ordering Example</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Examples">Examples</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#YouTube_JSON_API">YouTube JSON API</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Paging_Example">Paging Example</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Appendix">Appendix</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Appendix_A:_Reserved_JavaScript_Words">Appendix A: Reserved JavaScript Words</a></span>
</div></td>
</tr>
</tbody>
</table>

</div>

<div>

## Important Note

<div>

### <span id="Display_Hidden_Details_in_this_Guide">Display Hidden Details in this Guide</span>

<span id="link-Display_Hidden_Details_in_this_Guide__button" class="link_button"> [link](?showone=Display_Hidden_Details_in_this_Guide#Display_Hidden_Details_in_this_Guide) </span><span id="Display_Hidden_Details_in_this_Guide__button" class="showhide_button" onclick="javascript:ShowHideByName('Display_Hidden_Details_in_this_Guide')">▶</span>

<div style="display:inline;">

This style guide contains many details that are initially hidden from view. They are marked by the triangle icon, which you see here on your left. Click it now. You should see "Hooray" appear below.

</div>

<div>

<div id="Display_Hidden_Details_in_this_Guide__body" class="stylepoint_body" style="display: none">

Hooray! Now you know you can expand points to get more details. Alternatively, there's an "expand all" at the top of this document.

</div>

</div>

</div>

</div>

<div>

## Introduction

This style guide documents guidelines and recommendations for building JSON APIs at Google. In general, JSON APIs should follow the spec found at [JSON.org](https://www.json.org). This style guide clarifies and standardizes specific cases so that JSON APIs from Google have a standard look and feel. These guidelines are applicable to JSON requests and responses in both RPC-based and REST-based APIs.

</div>

<div>

## Definitions

For the purposes of this style guide, we define the following terms:

- property - a name/value pair inside a JSON object.
- property name - the name (or key) portion of the property.
- property value - the value portion of the property.

<div>

    {
      // The name/value pair together is a "property".
      "propertyName": "propertyValue"
    }

</div>

Javascript's `number` type encompasses all floating-point numbers, which is a broad designation. In this guide, `number` will refer to JavaScript's `number` type, while `integer` will refer to integers.

</div>

<div>

## General Guidelines

<div>

### <span id="Comments">Comments</span>

<span id="link-Comments__button" class="link_button"> [link](?showone=Comments#Comments) </span><span id="Comments__button" class="showhide_button" onclick="javascript:ShowHideByName('Comments')">▶</span>

<div style="display:inline;">

No comments in JSON objects.

</div>

<div>

<div id="Comments__body" class="stylepoint_body" style="display: none">

Comments should not be included in JSON objects. Some of the examples in this style guide include comments. However this is only to clarify the examples.

<div>

``` badcode
{
  // You may see comments in the examples below,
  // But don't include comments in your JSON.
  "propertyName": "propertyValue"
}
```

</div>

</div>

</div>

</div>

<div>

### <span id="Double_Quotes">Double Quotes</span>

<span id="link-Double_Quotes__button" class="link_button"> [link](?showone=Double_Quotes#Double_Quotes) </span><span id="Double_Quotes__button" class="showhide_button" onclick="javascript:ShowHideByName('Double_Quotes')">▶</span>

<div style="display:inline;">

Use double quotes.

</div>

<div>

<div id="Double_Quotes__body" class="stylepoint_body" style="display: none">

If a property requires quotes, double quotes must be used. All property names must be surrounded by double quotes. Property values of type string must be surrounded by double quotes. Other value types (like boolean or number) should not be surrounded by double quotes.

</div>

</div>

</div>

<div>

### <span id="Flattened_data_vs_Structured_Hierarchy">Flattened data vs Structured Hierarchy</span>

<span id="link-Flattened_data_vs_Structured_Hierarchy__button" class="link_button"> [link](?showone=Flattened_data_vs_Structured_Hierarchy#Flattened_data_vs_Structured_Hierarchy) </span><span id="Flattened_data_vs_Structured_Hierarchy__button" class="showhide_button" onclick="javascript:ShowHideByName('Flattened_data_vs_Structured_Hierarchy')">▶</span>

<div style="display:inline;">

Data should not be arbitrarily grouped for convenience.

</div>

<div>

<div id="Flattened_data_vs_Structured_Hierarchy__body" class="stylepoint_body" style="display: none">

Data elements should be "flattened" in the JSON representation. Data should not be arbitrarily grouped for convenience.

In some cases, such as a collection of properties that represents a single structure, it may make sense to keep the structured hierarchy. These cases should be carefully considered, and only used if it makes semantic sense. For example, an address could be represented two ways, but the structured way probably makes more sense for developers:

Flattened Address:

<div>

    {
      "company": "Google",
      "website": "https://www.google.com/",
      "addressLine1": "111 8th Ave",
      "addressLine2": "4th Floor",
      "state": "NY",
      "city": "New York",
      "zip": "10011"
    }

</div>

Structured Address:

<div>

    {
      "company": "Google",
      "website": "https://www.google.com/",
      "address": {
        "line1": "111 8th Ave",
        "line2": "4th Floor",
        "state": "NY",
        "city": "New York",
        "zip": "10011"
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Property Name Guidelines

<div>

### <span id="Property_Name_Format">Property Name Format</span>

<span id="link-Property_Name_Format__button" class="link_button"> [link](?showone=Property_Name_Format#Property_Name_Format) </span><span id="Property_Name_Format__button" class="showhide_button" onclick="javascript:ShowHideByName('Property_Name_Format')">▶</span>

<div style="display:inline;">

Choose meaningful property names.

</div>

<div>

<div id="Property_Name_Format__body" class="stylepoint_body" style="display: none">

Property names must conform to the following guidelines:

- Property names should be meaningful names with defined semantics.
- Property names must be camel-cased, ascii strings.
- The first character must be a letter, an underscore (\_) or a dollar sign (\$).
- Subsequent characters can be a letter, a digit, an underscore, or a dollar sign.
- Reserved JavaScript keywords should be avoided (A list of reserved JavaScript keywords can be found below).

These guidelines mirror the guidelines for naming JavaScript identifiers. This allows JavaScript clients to access properties using dot notation. (for example, `result.thisIsAnInstanceVariable`). Here's an example of an object with one property:

<div>

    {
      "thisPropertyIsAnIdentifier": "identifier value"
    }

</div>

</div>

</div>

</div>

<div>

### <span id="Key_Names_in_JSON_Maps">Key Names in JSON Maps</span>

<span id="link-Key_Names_in_JSON_Maps__button" class="link_button"> [link](?showone=Key_Names_in_JSON_Maps#Key_Names_in_JSON_Maps) </span><span id="Key_Names_in_JSON_Maps__button" class="showhide_button" onclick="javascript:ShowHideByName('Key_Names_in_JSON_Maps')">▶</span>

<div style="display:inline;">

JSON maps can use any Unicode character in key names.

</div>

<div>

<div id="Key_Names_in_JSON_Maps__body" class="stylepoint_body" style="display: none">

The property name naming rules do not apply when a JSON object is used as a map. A map (also referred to as an associative array) is a data type with arbitrary key/value pairs that use the keys to access the corresponding values. JSON objects and JSON maps look the same at runtime; this distinction is relevant to the design of the API. The API documentation should indicate when JSON objects are used as maps.

The keys of a map do not have to obey the naming guidelines for property names. Map keys may contain any Unicode characters. Clients can access these properties using the square bracket notation familiar for maps (for example, `result.thumbnails["72"]`).

<div>

    {
      // The "address" property is a sub-object
      // holding the parts of an address.
      "address": {
        "addressLine1": "123 Anystreet",
        "city": "Anytown",
        "state": "XX",
        "zip": "00000"
      },
      // The "thumbnails" property is a map that maps
      // a pixel size to the thumbnail url of that size.
      "thumbnails": {
        "72": "https://url.to.72px.thumbnail",
        "144": "https://url.to.144px.thumbnail"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="Reserved_Property_Names">Reserved Property Names</span>

<span id="link-Reserved_Property_Names__button" class="link_button"> [link](?showone=Reserved_Property_Names#Reserved_Property_Names) </span><span id="Reserved_Property_Names__button" class="showhide_button" onclick="javascript:ShowHideByName('Reserved_Property_Names')">▶</span>

<div style="display:inline;">

Certain property names are reserved for consistent use across services.

</div>

<div>

<div id="Reserved_Property_Names__body" class="stylepoint_body" style="display: none">

Details about reserved property names, along with the full list, can be found later on in this guide. Services should avoid using these property names for anything other than their defined semantics.

</div>

</div>

</div>

<div>

### <span id="Singular_vs_Plural_Property_Names">Singular vs Plural Property Names</span>

<span id="link-Singular_vs_Plural_Property_Names__button" class="link_button"> [link](?showone=Singular_vs_Plural_Property_Names#Singular_vs_Plural_Property_Names) </span><span id="Singular_vs_Plural_Property_Names__button" class="showhide_button" onclick="javascript:ShowHideByName('Singular_vs_Plural_Property_Names')">▶</span>

<div style="display:inline;">

Array types should have plural property names. All other property names should be singular.

</div>

<div>

<div id="Singular_vs_Plural_Property_Names__body" class="stylepoint_body" style="display: none">

Arrays usually contain multiple items, and a plural property name reflects this. An example of this can be seen in the reserved names below. The `items` property name is plural because it represents an array of item objects. Most of the other fields are singular.

There may be exceptions to this, especially when referring to numeric property values. For example, in the reserved names, `totalItems` makes more sense than `totalItem`. However, technically, this is not violating the style guide, since `totalItems` can be viewed as `totalOfItems`, where `total` is singular (as per the style guide), and `OfItems` serves to qualify the total. The field name could also be changed to `itemCount` to look singular.

<div>

    {
      // Singular
      "author": "lisa",
      // An array of siblings, plural
      "siblings": [ "bart", "maggie"],
      // "totalItem" doesn't sound right
      "totalItems": 10,
      // But maybe "itemCount" is better
      "itemCount": 10,
    }

</div>

</div>

</div>

</div>

<div>

### <span id="Naming_Conflicts">Naming Conflicts</span>

<span id="link-Naming_Conflicts__button" class="link_button"> [link](?showone=Naming_Conflicts#Naming_Conflicts) </span><span id="Naming_Conflicts__button" class="showhide_button" onclick="javascript:ShowHideByName('Naming_Conflicts')">▶</span>

<div style="display:inline;">

Avoid naming conflicts by choosing a new property name or versioning the API.

</div>

<div>

<div id="Naming_Conflicts__body" class="stylepoint_body" style="display: none">

New properties may be added to the reserved list in the future. There is no concept of JSON namespacing. If there is a naming conflict, these can usually be resolved by choosing a new property name or by versioning. For example, suppose we start with the following JSON object:

<div>

    {
      "apiVersion": "1.0",
      "data": {
        "recipeName": "pizza",
        "ingredients": ["tomatoes", "cheese", "sausage"]
      }
    }

</div>

If in the future we wish to make `ingredients` a reserved word, we can do one of two things:

1\) Choose a different name:

<div>

    {
      "apiVersion": "1.0",
      "data": {
        "recipeName": "pizza",
        "ingredientsData": "Some new property",
        "ingredients": ["tomatoes", "cheese", "sausage"]
      }
    }

</div>

2\) Rename the property on a major version boundary:

<div>

    {
      "apiVersion": "2.0",
      "data": {
        "recipeName": "pizza",
        "ingredients": "Some new property",
        "recipeIngredients": ["tomatos", "cheese", "sausage"]
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Property Value Guidelines

<div>

### <span id="Property_Value_Format">Property Value Format</span>

<span id="link-Property_Value_Format__button" class="link_button"> [link](?showone=Property_Value_Format#Property_Value_Format) </span><span id="Property_Value_Format__button" class="showhide_button" onclick="javascript:ShowHideByName('Property_Value_Format')">▶</span>

<div style="display:inline;">

Property values must be booleans, numbers, Unicode strings, objects, arrays, or `null`.

</div>

<div>

<div id="Property_Value_Format__body" class="stylepoint_body" style="display: none">

The spec at [JSON.org](https://www.json.org) specifies exactly what type of data is allowed in a property value. This includes booleans, numbers, Unicode strings, objects, arrays, and `null`. JavaScript expressions are not allowed. APIs should support that spec for all values, and should choose the data type most appropriate for a particular property (numbers to represent numbers, etc.).

Good:

<div>

    {
      "canPigsFly": null,     // null
      "areWeThereYet": false, // boolean
      "answerToLife": 42,     // number
      "name": "Bart",         // string
      "moreData": {},         // object
      "things": []            // array
    }

</div>

Bad:

<div>

``` badcode
{
  "aVariableName": aVariableName,         // Bad - JavaScript identifier
  "functionFoo": function() { return 1; } // Bad - JavaScript function
}
```

</div>

</div>

</div>

</div>

<div>

### <span id="Empty/Null_Property_Values">Empty/Null Property Values</span>

<span id="link-Empty/Null_Property_Values__button" class="link_button"> [link](?showone=Empty/Null_Property_Values#Empty/Null_Property_Values) </span><span id="Empty/Null_Property_Values__button" class="showhide_button" onclick="javascript:ShowHideByName('Empty/Null_Property_Values')">▶</span>

<div style="display:inline;">

Consider removing empty or `null` values.

</div>

<div>

<div id="Empty/Null_Property_Values__body" class="stylepoint_body" style="display: none">

If a property is optional or has an empty or `null` value, consider dropping the property from the JSON, unless there's a strong semantic reason for its existence.

<div>

    {
      "volume": 10,

      // Even though the "balance" property's value is zero, it should be left in,
      // since "0" signifies "even balance" (the value could be "-1" for left
      // balance and "+1" for right balance.
      "balance": 0,

      // The "currentlyPlaying" property can be left out since it is null.
      // "currentlyPlaying": null
    }

</div>

</div>

</div>

</div>

<div>

### <span id="Enum_Values">Enum Values</span>

<span id="link-Enum_Values__button" class="link_button"> [link](?showone=Enum_Values#Enum_Values) </span><span id="Enum_Values__button" class="showhide_button" onclick="javascript:ShowHideByName('Enum_Values')">▶</span>

<div style="display:inline;">

Enum values should be represented as strings.

</div>

<div>

<div id="Enum_Values__body" class="stylepoint_body" style="display: none">

As APIs grow, enum values may be added, removed or changed. Using strings as enum values ensures that downstream clients can gracefully handle changes to enum values.

Java code:

<div>

    public enum Color {
      WHITE,
      BLACK,
      RED,
      YELLOW,
      BLUE
    }

</div>

JSON object:

<div>

    {
      "color": "WHITE"
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Property Value Data Types

As mentioned above, property value types must be booleans, numbers, strings, objects, arrays, or `null`. However, it is useful define a set of standard data types when dealing with certain values. These data types will always be strings, but they will be formatted in a specific manner so that they can be easily parsed.

<div>

### <span id="Date_Property_Values">Date Property Values</span>

<span id="link-Date_Property_Values__button" class="link_button"> [link](?showone=Date_Property_Values#Date_Property_Values) </span><span id="Date_Property_Values__button" class="showhide_button" onclick="javascript:ShowHideByName('Date_Property_Values')">▶</span>

<div style="display:inline;">

Dates should be formatted as recommended by RFC 3339.

</div>

<div>

<div id="Date_Property_Values__body" class="stylepoint_body" style="display: none">

Dates should be strings formatted as recommended by [RFC 3339](https://www.ietf.org/rfc/rfc3339.txt)

<div>

    {
      "lastUpdate": "2007-11-06T16:34:41.000Z"
    }

</div>

</div>

</div>

</div>

<div>

### <span id="Time_Duration_Property_Values">Time Duration Property Values</span>

<span id="link-Time_Duration_Property_Values__button" class="link_button"> [link](?showone=Time_Duration_Property_Values#Time_Duration_Property_Values) </span><span id="Time_Duration_Property_Values__button" class="showhide_button" onclick="javascript:ShowHideByName('Time_Duration_Property_Values')">▶</span>

<div style="display:inline;">

Time durations should be formatted as recommended by ISO 8601.

</div>

<div>

<div id="Time_Duration_Property_Values__body" class="stylepoint_body" style="display: none">

Time duration values should be strings formatted as recommended by [ISO 8601](https://en.wikipedia.org/wiki/ISO_8601#Durations).

<div>

    {
      // three years, six months, four days, twelve hours,
      // thirty minutes, and five seconds
      "duration": "P3Y6M4DT12H30M5S"
    }

</div>

</div>

</div>

</div>

<div>

### <span id="Latitude/Longitude_Property_Values">Latitude/Longitude Property Values</span>

<span id="link-Latitude/Longitude_Property_Values__button" class="link_button"> [link](?showone=Latitude/Longitude_Property_Values#Latitude/Longitude_Property_Values) </span><span id="Latitude/Longitude_Property_Values__button" class="showhide_button" onclick="javascript:ShowHideByName('Latitude/Longitude_Property_Values')">▶</span>

<div style="display:inline;">

Latitudes/Longitudes should be formatted as recommended by ISO 6709.

</div>

<div>

<div id="Latitude/Longitude_Property_Values__body" class="stylepoint_body" style="display: none">

Latitude/Longitude should be strings formatted as recommended by [ISO 6709](https://en.wikipedia.org/wiki/ISO_6709). Furthermore, they should favor the ±DD.DDDD±DDD.DDDD degrees format.

<div>

    {
      // The latitude/longitude location of the statue of liberty.
      "statueOfLiberty": "+40.6894-074.0447"
    }

</div>

</div>

</div>

</div>

</div>

<div>

## JSON Structure & Reserved Property Names

In order to maintain a consistent interface across APIs, JSON objects should follow the structure outlined below. This structure applies to both requests and responses made with JSON. Within this structure, there are certain property names that are reserved for specific uses. These properties are NOT required; in other words, each reserved property may appear zero or one times. But if a service needs these properties, this naming convention is recommended. Here is a schema of the JSON structure, represented in [Orderly](https://www.google.com/url?sa=D&q=http%3A%2F%2Forderly-json.org%2F) format (which in turn can be compiled into a [JSONSchema](https://www.google.com/url?sa=D&q=http%3A%2F%2Fjson-schema.org%2F)). You can few examples of the JSON structure at the end of this guide.

<div>

    object {
      string apiVersion?;
      string context?;
      string id?;
      string method?;
      object {
        string id?
      }* params?;
      object {
        string kind?;
        string fields?;
        string etag?;
        string id?;
        string lang?;
        string updated?; # date formatted RFC 3339
        boolean deleted?;
        integer currentItemCount?;
        integer itemsPerPage?;
        integer startIndex?;
        integer totalItems?;
        integer pageIndex?;
        integer totalPages?;
        string pageLinkTemplate /^https?:/ ?;
        object {}* next?;
        string nextLink?;
        object {}* previous?;
        string previousLink?;
        object {}* self?;
        string selfLink?;
        object {}* edit?;
        string editLink?;
        array [
          object {}*;
        ] items?;
      }* data?;
      object {
        integer code?;
        string message?;
        array [
          object {
            string domain?;
            string reason?;
            string message?;
            string location?;
            string locationType?;
            string extendedHelp?;
            string sendReport?;
          }*;
        ] errors?;
      }* error?;
    }*;

</div>

The JSON object has a few top-level properties, followed by either a `data` object or an `error` object, but not both. An explanation of each of these properties can be found below.

</div>

<div>

## Top-Level Reserved Property Names

The top-level of the JSON object may contain the following properties.

<div>

### <span id="apiVersion">apiVersion</span>

<span id="link-apiVersion__button" class="link_button"> [link](?showone=apiVersion#apiVersion) </span><span id="apiVersion__button" class="showhide_button" onclick="javascript:ShowHideByName('apiVersion')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: -

</div>

<div>

<div id="apiVersion__body" class="stylepoint_body" style="display: none">

Represents the desired version of the service API in a request, and the version of the service API that's served in the response. `apiVersion` should always be present. This is not related to the version of the data. Versioning of data should be handled through some other mechanism such as etags.

Example:

<div>

    { "apiVersion": "2.1" }

</div>

</div>

</div>

</div>

<div>

### <span id="context">context</span>

<span id="link-context__button" class="link_button"> [link](?showone=context#context) </span><span id="context__button" class="showhide_button" onclick="javascript:ShowHideByName('context')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: -

</div>

<div>

<div id="context__body" class="stylepoint_body" style="display: none">

Client sets this value and server echos data in the response. This is useful in JSON-P and batch situations , where the user can use the `context` to correlate responses with requests. This property is a top-level property because the `context` should present regardless of whether the response was successful or an error. `context` differs from `id` in that `context` is specified by the user while `id` is assigned by the service.

Example:

Request \#1:

<div>

    https://www.google.com/myapi?context=bart

</div>

Request \#2:

<div>

    https://www.google.com/myapi?context=lisa

</div>

Response \#1:

<div>

    {
      "context": "bart",
      "data": {
        "items": []
      }
    }

</div>

Response \#2:

<div>

    {
      "context": "lisa",
      "data": {
        "items": []
      }
    }

</div>

Common JavaScript handler code to process both responses:

<div>

    function handleResponse(response) {
      if (response.result.context == "bart") {
        // Update the "Bart" section of the page.
      } else if (response.result.context == "lisa") {
        // Update the "Lisa" section of the page.
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="id">id</span>

<span id="link-id__button" class="link_button"> [link](?showone=id#id) </span><span id="id__button" class="showhide_button" onclick="javascript:ShowHideByName('id')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: -

</div>

<div>

<div id="id__body" class="stylepoint_body" style="display: none">

A server supplied identifier for the response (regardless of whether the response is a success or an error). This is useful for correlating server logs with individual responses received at a client.

Example:

<div>

    { "id": "1" }

</div>

</div>

</div>

</div>

<div>

### <span id="method">method</span>

<span id="link-method__button" class="link_button"> [link](?showone=method#method) </span><span id="method__button" class="showhide_button" onclick="javascript:ShowHideByName('method')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: -

</div>

<div>

<div id="method__body" class="stylepoint_body" style="display: none">

Represents the operation to perform, or that was performed, on the data. In the case of a JSON request, the `method` property can be used to indicate which operation to perform on the data. In the case of a JSON response, the `method` property can indicate the operation performed on the data.

One example of this is in JSON-RPC requests, where `method` indicates the operation to perform on the `params` property:

<div>

    {
      "method": "people.get",
      "params": {
        "userId": "@me",
        "groupId": "@self"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="params">params</span>

<span id="link-params__button" class="link_button"> [link](?showone=params#params) </span><span id="params__button" class="showhide_button" onclick="javascript:ShowHideByName('params')">▶</span>

<div style="display:inline;">

Property Value Type: object\
Parent: -

</div>

<div>

<div id="params__body" class="stylepoint_body" style="display: none">

This object serves as a map of input parameters to send to an RPC request. It can be used in conjunction with the `method` property to execute an RPC function. If an RPC function does not need parameters, this property can be omitted.

Example:

<div>

    {
      "method": "people.get",
      "params": {
        "userId": "@me",
        "groupId": "@self"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data">data</span>

<span id="link-data__button" class="link_button"> [link](?showone=data#data) </span><span id="data__button" class="showhide_button" onclick="javascript:ShowHideByName('data')">▶</span>

<div style="display:inline;">

Property Value Type: object\
Parent: -

</div>

<div>

<div id="data__body" class="stylepoint_body" style="display: none">

Container for all the data from a response. This property itself has many reserved property names, which are described below. Services are free to add their own data to this object. A JSON response should contain either a `data` object or an `error` object, but not both. If both `data` and `error` are present, the `error` object takes precedence.

</div>

</div>

</div>

<div>

### <span id="error">error</span>

<span id="link-error__button" class="link_button"> [link](?showone=error#error) </span><span id="error__button" class="showhide_button" onclick="javascript:ShowHideByName('error')">▶</span>

<div style="display:inline;">

Property Value Type: object\
Parent: -

</div>

<div>

<div id="error__body" class="stylepoint_body" style="display: none">

Indicates that an error has occurred, with details about the error. The error format supports either one or more errors returned from the service. A JSON response should contain either a `data` object or an `error` object, but not both. If both `data` and `error` are present, the `error` object takes precedence.

Example:

<div>

    {
      "apiVersion": "2.0",
      "error": {
        "code": 404,
        "message": "File Not Found",
        "errors": [{
          "domain": "Calendar",
          "reason": "ResourceNotFoundException",
          "message": "File Not Found"
        }]
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Reserved Property Names in the data object

The `data` property of the JSON object may contain the following properties.

<div>

### <span id="data.kind">data.kind</span>

<span id="link-data.kind__button" class="link_button"> [link](?showone=data.kind#data.kind) </span><span id="data.kind__button" class="showhide_button" onclick="javascript:ShowHideByName('data.kind')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `data`

</div>

<div>

<div id="data.kind__body" class="stylepoint_body" style="display: none">

The `kind` property serves as a guide to what type of information this particular object stores. It can be present at the `data` level, or at the `items` level, or in any object where its helpful to distinguish between various types of objects. If the `kind` object is present, it should be the first property in the object (See the "Property Ordering" section below for more details).

Example:

<div>

    // "Kind" indicates an "album" in the Picasa API.
    {"data": {"kind": "album"}}

</div>

</div>

</div>

</div>

<div>

### <span id="data.fields">data.fields</span>

<span id="link-data.fields__button" class="link_button"> [link](?showone=data.fields#data.fields) </span><span id="data.fields__button" class="showhide_button" onclick="javascript:ShowHideByName('data.fields')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `data`

</div>

<div>

<div id="data.fields__body" class="stylepoint_body" style="display: none">

Represents the fields present in the response when doing a partial GET, or the fields present in a request when doing a partial PATCH. This property should only exist during a partial GET/PATCH, and should not be empty.

Example:

<div>

    {
      "data": {
        "kind": "user",
        "fields": "author,id",
        "id": "bart",
        "author": "Bart"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.etag">data.etag</span>

<span id="link-data.etag__button" class="link_button"> [link](?showone=data.etag#data.etag) </span><span id="data.etag__button" class="showhide_button" onclick="javascript:ShowHideByName('data.etag')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `data`

</div>

<div>

<div id="data.etag__body" class="stylepoint_body" style="display: none">

Represents the etag for the response. Details about ETags in the GData APIs can be found here: <https://code.google.com/apis/gdata/docs/2.0/reference.html#ResourceVersioning>

Example:

<div>

    {"data": {"etag": "W/"C0QBRXcycSp7ImA9WxRVFUk.""}}

</div>

</div>

</div>

</div>

<div>

### <span id="data.id">data.id</span>

<span id="link-data.id__button" class="link_button"> [link](?showone=data.id#data.id) </span><span id="data.id__button" class="showhide_button" onclick="javascript:ShowHideByName('data.id')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `data`

</div>

<div>

<div id="data.id__body" class="stylepoint_body" style="display: none">

A globally unique string used to reference the object. The specific details of the `id` property are left up to the service.

Example:

<div>

    {"data": {"id": "12345"}}

</div>

</div>

</div>

</div>

<div>

### <span id="data.lang">data.lang</span>

<span id="link-data.lang__button" class="link_button"> [link](?showone=data.lang#data.lang) </span><span id="data.lang__button" class="showhide_button" onclick="javascript:ShowHideByName('data.lang')">▶</span>

<div style="display:inline;">

Property Value Type: string (formatted as specified in BCP 47)\
Parent: `data (or any child element)`

</div>

<div>

<div id="data.lang__body" class="stylepoint_body" style="display: none">

Indicates the language of the rest of the properties in this object. This property mimics HTML's `lang` property and XML's `xml:lang` properties. The value should be a language value as defined in [BCP 47](https://www.rfc-editor.org/rfc/bcp/bcp47.txt). If a single JSON object contains data in multiple languages, the service is responsible for developing and documenting an appropriate location for the `lang` property.

Example:

<div>

    {"data": {
      "items": [
        { "lang": "en",
          "title": "Hello world!" },
        { "lang": "fr",
          "title": "Bonjour monde!" }
      ]}
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.updated">data.updated</span>

<span id="link-data.updated__button" class="link_button"> [link](?showone=data.updated#data.updated) </span><span id="data.updated__button" class="showhide_button" onclick="javascript:ShowHideByName('data.updated')">▶</span>

<div style="display:inline;">

Property Value Type: string (formatted as specified in RFC 3339)\
Parent: `data`

</div>

<div>

<div id="data.updated__body" class="stylepoint_body" style="display: none">

Indicates the last date/time ([RFC 3339](https://www.ietf.org/rfc/rfc3339.txt)) the item was updated, as defined by the service.

Example:

<div>

    {"data": {"updated": "2007-11-06T16:34:41.000Z"}}

</div>

</div>

</div>

</div>

<div>

### <span id="data.deleted">data.deleted</span>

<span id="link-data.deleted__button" class="link_button"> [link](?showone=data.deleted#data.deleted) </span><span id="data.deleted__button" class="showhide_button" onclick="javascript:ShowHideByName('data.deleted')">▶</span>

<div style="display:inline;">

Property Value Type: boolean\
Parent: `data (or any child element)`

</div>

<div>

<div id="data.deleted__body" class="stylepoint_body" style="display: none">

A marker element, that, when present, indicates the containing entry is deleted. If deleted is present, its value must be `true`; a value of `false` can cause confusion and should be avoided.

Example:

<div>

    {"data": {
      "items": [
        { "title": "A deleted entry",
          "deleted": true
        }
      ]}
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.items">data.items</span>

<span id="link-data.items__button" class="link_button"> [link](?showone=data.items#data.items) </span><span id="data.items__button" class="showhide_button" onclick="javascript:ShowHideByName('data.items')">▶</span>

<div style="display:inline;">

Property Value Type: array\
Parent: `data`

</div>

<div>

<div id="data.items__body" class="stylepoint_body" style="display: none">

The property name `items` is reserved to represent an array of items (for example, photos in Picasa, videos in YouTube). This construct is intended to provide a standard location for collections related to the current result. For example, the JSON output could be plugged into a generic pagination system that knows to page on the `items` array. If `items` exists, it should be the last property in the `data` object (See the "Property Ordering" section below for more details).

Example:

<div>

    {
      "data": {
        "items": [
          { /* Object #1 */ },
          { /* Object #2 */ },
          ...
        ]
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Reserved Property Names for Paging

The following properties are located in the `data` object, and help page through a list of items. Some of the language and concepts are borrowed from the [OpenSearch specification](https://www.opensearch.org/).

The paging properties below allow for various styles of paging, including:

- Previous/Next paging - Allows user's to move forward and backward through a list, one page at a time. The `nextLink` and `previousLink` properties (described in the "Reserved Property Names for Links" section below) are used for this style of paging.
- Index-based paging - Allows user's to jump directly to a specific item position within a list of items. For example, to load 10 items starting at item 200, the developer may point the user to a url with the query string `?startIndex=200`.
- Page-based paging - Allows user's to jump directly to a specific page within the items. This is similar to index-based paging, but saves the developer the extra step of having to calculate the item index for a new page of items. For example, rather than jump to item number 200, the developer could jump to page 20. The urls during page-based paging could use the query string `?page=1` or `?page=20`. The `pageIndex` and `totalPages` properties are used for this style of paging.

An example of how to use these properties to implement paging can be found at the end of this guide.

<div>

### <span id="data.currentItemCount">data.currentItemCount</span>

<span id="link-data.currentItemCount__button" class="link_button"> [link](?showone=data.currentItemCount#data.currentItemCount) </span><span id="data.currentItemCount__button" class="showhide_button" onclick="javascript:ShowHideByName('data.currentItemCount')">▶</span>

<div style="display:inline;">

Property Value Type: integer\
Parent: `data`

</div>

<div>

<div id="data.currentItemCount__body" class="stylepoint_body" style="display: none">

The number of items in this result set. Should be equivalent to items.length, and is provided as a convenience property. For example, suppose a developer requests a set of search items, and asks for 10 items per page. The total set of that search has 14 total items. The first page of items will have 10 items in it, so both `itemsPerPage` and `currentItemCount` will equal "10". The next page of items will have the remaining 4 items; `itemsPerPage` will still be "10", but `currentItemCount` will be "4".

Example:

<div>

    {
      "data": {
        // "itemsPerPage" does not necessarily match "currentItemCount"
        "itemsPerPage": 10,
        "currentItemCount": 4
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.itemsPerPage">data.itemsPerPage</span>

<span id="link-data.itemsPerPage__button" class="link_button"> [link](?showone=data.itemsPerPage#data.itemsPerPage) </span><span id="data.itemsPerPage__button" class="showhide_button" onclick="javascript:ShowHideByName('data.itemsPerPage')">▶</span>

<div style="display:inline;">

Property Value Type: integer\
Parent: `data`

</div>

<div>

<div id="data.itemsPerPage__body" class="stylepoint_body" style="display: none">

The number of items in the result. This is not necessarily the size of the data.items array; if we are viewing the last page of items, the size of data.items may be less than `itemsPerPage`. However the size of data.items should not exceed `itemsPerPage`.

Example:

<div>

    {
      "data": {
        "itemsPerPage": 10
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.startIndex">data.startIndex</span>

<span id="link-data.startIndex__button" class="link_button"> [link](?showone=data.startIndex#data.startIndex) </span><span id="data.startIndex__button" class="showhide_button" onclick="javascript:ShowHideByName('data.startIndex')">▶</span>

<div style="display:inline;">

Property Value Type: integer\
Parent: `data`

</div>

<div>

<div id="data.startIndex__body" class="stylepoint_body" style="display: none">

The index of the first item in data.items. For consistency, `startIndex` should be 1-based. For example, the first item in the first set of items should have a `startIndex` of 1. If the user requests the next set of data, the `startIndex` may be 10.

Example:

<div>

    {
      "data": {
        "startIndex": 1
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.totalItems">data.totalItems</span>

<span id="link-data.totalItems__button" class="link_button"> [link](?showone=data.totalItems#data.totalItems) </span><span id="data.totalItems__button" class="showhide_button" onclick="javascript:ShowHideByName('data.totalItems')">▶</span>

<div style="display:inline;">

Property Value Type: integer\
Parent: `data`

</div>

<div>

<div id="data.totalItems__body" class="stylepoint_body" style="display: none">

The total number of items available in this set. For example, if a user has 100 blog posts, the response may only contain 10 items, but the `totalItems` would be 100.

Example:

<div>

    {
      "data": {
        "totalItems": 100
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.pagingLinkTemplate">data.pagingLinkTemplate</span>

<span id="link-data.pagingLinkTemplate__button" class="link_button"> [link](?showone=data.pagingLinkTemplate#data.pagingLinkTemplate) </span><span id="data.pagingLinkTemplate__button" class="showhide_button" onclick="javascript:ShowHideByName('data.pagingLinkTemplate')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `data`

</div>

<div>

<div id="data.pagingLinkTemplate__body" class="stylepoint_body" style="display: none">

A URI template indicating how users can calculate subsequent paging links. The URI template also has some reserved variable names: `{index}` representing the item number to load, and `{pageIndex}`, representing the page number to load.

Example:

<div>

    {
      "data": {
        "pagingLinkTemplate": "https://www.google.com/search/hl=en&q=chicago+style+pizza&start={index}&sa=N"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.pageIndex">data.pageIndex</span>

<span id="link-data.pageIndex__button" class="link_button"> [link](?showone=data.pageIndex#data.pageIndex) </span><span id="data.pageIndex__button" class="showhide_button" onclick="javascript:ShowHideByName('data.pageIndex')">▶</span>

<div style="display:inline;">

Property Value Type: integer\
Parent: `data`

</div>

<div>

<div id="data.pageIndex__body" class="stylepoint_body" style="display: none">

The index of the current page of items. For consistency, `pageIndex` should be 1-based. For example, the first page of items has a `pageIndex` of 1. `pageIndex` can also be calculated from the item-based paging properties: `pageIndex = floor(startIndex / itemsPerPage) + 1`.

Example:

<div>

    {
      "data": {
        "pageIndex": 1
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.totalPages">data.totalPages</span>

<span id="link-data.totalPages__button" class="link_button"> [link](?showone=data.totalPages#data.totalPages) </span><span id="data.totalPages__button" class="showhide_button" onclick="javascript:ShowHideByName('data.totalPages')">▶</span>

<div style="display:inline;">

Property Value Type: integer\
Parent: `data`

</div>

<div>

<div id="data.totalPages__body" class="stylepoint_body" style="display: none">

The total number of pages in the result set. `totalPages` can also be calculated from the item-based paging properties above: `totalPages = ceiling(totalItems / itemsPerPage)`.

Example:

<div>

    {
      "data": {
        "totalPages": 50
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Reserved Property Names for Links

The following properties are located in the `data` object, and represent references to other resources. There are two forms of link properties: 1) objects, which can contain any sort of reference (such as a JSON-RPC object), and 2) URI strings, which represent URIs to resources (and will always be suffixed with "Link").

<div>

### <span id="data.self_/_data.selfLink">data.self / data.selfLink</span>

<span id="link-data.self_/_data.selfLink__button" class="link_button"> [link](?showone=data.self_/_data.selfLink#data.self_/_data.selfLink) </span><span id="data.self_/_data.selfLink__button" class="showhide_button" onclick="javascript:ShowHideByName('data.self_/_data.selfLink')">▶</span>

<div style="display:inline;">

Property Value Type: object / string\
Parent: `data`

</div>

<div>

<div id="data.self_/_data.selfLink__body" class="stylepoint_body" style="display: none">

The self link can be used to retrieve the item's data. For example, in a list of a user's Picasa album, each album object in the `items` array could contain a `selfLink` that can be used to retrieve data related to that particular album.

Example:

<div>

    {
      "data": {
        "self": { },
        "selfLink": "https://www.google.com/feeds/album/1234"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.edit_/_data.editLink">data.edit / data.editLink</span>

<span id="link-data.edit_/_data.editLink__button" class="link_button"> [link](?showone=data.edit_/_data.editLink#data.edit_/_data.editLink) </span><span id="data.edit_/_data.editLink__button" class="showhide_button" onclick="javascript:ShowHideByName('data.edit_/_data.editLink')">▶</span>

<div style="display:inline;">

Property Value Type: object / string\
Parent: `data`

</div>

<div>

<div id="data.edit_/_data.editLink__body" class="stylepoint_body" style="display: none">

The edit link indicates where a user can send update or delete requests. This is useful for REST-based APIs. This link need only be present if the user can update/delete this item.

Example:

<div>

    {
      "data": {
        "edit": { },
        "editLink": "https://www.google.com/feeds/album/1234/edit"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.next_/_data.nextLink">data.next / data.nextLink</span>

<span id="link-data.next_/_data.nextLink__button" class="link_button"> [link](?showone=data.next_/_data.nextLink#data.next_/_data.nextLink) </span><span id="data.next_/_data.nextLink__button" class="showhide_button" onclick="javascript:ShowHideByName('data.next_/_data.nextLink')">▶</span>

<div style="display:inline;">

Property Value Type: object / string\
Parent: `data`

</div>

<div>

<div id="data.next_/_data.nextLink__body" class="stylepoint_body" style="display: none">

The next link indicates how more data can be retrieved. It points to the location to load the next set of data. It can be used in conjunction with the `itemsPerPage`, `startIndex` and `totalItems` properties in order to page through data.

Example:

<div>

    {
      "data": {
        "next": { },
        "nextLink": "https://www.google.com/feeds/album/1234/next"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="data.previous_/_data.previousLink">data.previous / data.previousLink</span>

<span id="link-data.previous_/_data.previousLink__button" class="link_button"> [link](?showone=data.previous_/_data.previousLink#data.previous_/_data.previousLink) </span><span id="data.previous_/_data.previousLink__button" class="showhide_button" onclick="javascript:ShowHideByName('data.previous_/_data.previousLink')">▶</span>

<div style="display:inline;">

Property Value Type: object / string\
Parent: `data`

</div>

<div>

<div id="data.previous_/_data.previousLink__body" class="stylepoint_body" style="display: none">

The previous link indicates how more data can be retrieved. It points to the location to load the previous set of data. It can be used in conjunction with the `itemsPerPage`, `startIndex` and `totalItems` properties in order to page through data.

Example:

<div>

    {
      "data": {
        "previous": { },
        "previousLink": "https://www.google.com/feeds/album/1234/next"
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Reserved Property Names in the error object

The `error` property of the JSON object may contain the following properties.

<div>

### <span id="error.code">error.code</span>

<span id="link-error.code__button" class="link_button"> [link](?showone=error.code#error.code) </span><span id="error.code__button" class="showhide_button" onclick="javascript:ShowHideByName('error.code')">▶</span>

<div style="display:inline;">

Property Value Type: integer\
Parent: `error`

</div>

<div>

<div id="error.code__body" class="stylepoint_body" style="display: none">

Represents the code for this error. This property value will usually represent the HTTP response code. If there are multiple errors, `code` will be the error code for the first error.

Example:

<div>

    {
      "error":{
        "code": 404
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.message">error.message</span>

<span id="link-error.message__button" class="link_button"> [link](?showone=error.message#error.message) </span><span id="error.message__button" class="showhide_button" onclick="javascript:ShowHideByName('error.message')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error`

</div>

<div>

<div id="error.message__body" class="stylepoint_body" style="display: none">

A human readable message providing more details about the error. If there are multiple errors, `message` will be the message for the first error.

Example:

<div>

    {
      "error":{
        "message": "File Not Found"
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors">error.errors</span>

<span id="link-error.errors__button" class="link_button"> [link](?showone=error.errors#error.errors) </span><span id="error.errors__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors')">▶</span>

<div style="display:inline;">

Property Value Type: array\
Parent: `error`

</div>

<div>

<div id="error.errors__body" class="stylepoint_body" style="display: none">

Container for any additional information regarding the error. If the service returns multiple errors, each element in the `errors` array represents a different error.

Example:

<div>

    { "error": { "errors": [] } }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors[].domain">error.errors\[\].domain</span>

<span id="link-error.errors[].domain__button" class="link_button"> [link](?showone=error.errors%5B%5D.domain#error.errors%5B%5D.domain) </span><span id="error.errors[].domain__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors[].domain')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error.errors`

</div>

<div>

<div id="error.errors[].domain__body" class="stylepoint_body" style="display: none">

Unique identifier for the service raising this error. This helps distinguish service-specific errors (i.e. error inserting an event in a calendar) from general protocol errors (i.e. file not found).

Example:

<div>

    {
      "error":{
        "errors": [{"domain": "Calendar"}]
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors[].reason">error.errors\[\].reason</span>

<span id="link-error.errors[].reason__button" class="link_button"> [link](?showone=error.errors%5B%5D.reason#error.errors%5B%5D.reason) </span><span id="error.errors[].reason__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors[].reason')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error.errors`

</div>

<div>

<div id="error.errors[].reason__body" class="stylepoint_body" style="display: none">

Unique identifier for this error. Different from the `error.code` property in that this is not an http response code.

Example:

<div>

    {
      "error":{
        "errors": [{"reason": "ResourceNotFoundException"}]
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors[].message">error.errors\[\].message</span>

<span id="link-error.errors[].message__button" class="link_button"> [link](?showone=error.errors%5B%5D.message#error.errors%5B%5D.message) </span><span id="error.errors[].message__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors[].message')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error.errors`

</div>

<div>

<div id="error.errors[].message__body" class="stylepoint_body" style="display: none">

A human readable message providing more details about the error. If there is only one error, this field will match `error.message`.

Example:

<div>

    {
      "error":{
        "code": 404,
        "message": "File Not Found",
        "errors": [{"message": "File Not Found"}]
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors[].location">error.errors\[\].location</span>

<span id="link-error.errors[].location__button" class="link_button"> [link](?showone=error.errors%5B%5D.location#error.errors%5B%5D.location) </span><span id="error.errors[].location__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors[].location')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error.errors`

</div>

<div>

<div id="error.errors[].location__body" class="stylepoint_body" style="display: none">

The location of the error (the interpretation of its value depends on `locationType`).

Example:

<div>

    {
      "error":{
        "errors": [{"location": ""}]
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors[].locationType">error.errors\[\].locationType</span>

<span id="link-error.errors[].locationType__button" class="link_button"> [link](?showone=error.errors%5B%5D.locationType#error.errors%5B%5D.locationType) </span><span id="error.errors[].locationType__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors[].locationType')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error.errors`

</div>

<div>

<div id="error.errors[].locationType__body" class="stylepoint_body" style="display: none">

Indicates how the `location` property should be interpreted.

Example:

<div>

    {
      "error":{
        "errors": [{"locationType": ""}]
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors[].extendedHelp">error.errors\[\].extendedHelp</span>

<span id="link-error.errors[].extendedHelp__button" class="link_button"> [link](?showone=error.errors%5B%5D.extendedHelp#error.errors%5B%5D.extendedHelp) </span><span id="error.errors[].extendedHelp__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors[].extendedHelp')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error.errors`

</div>

<div>

<div id="error.errors[].extendedHelp__body" class="stylepoint_body" style="display: none">

A URI for a help text that might shed some more light on the error.

Example:

<div>

    {
      "error":{
        "errors": [{"extendedHelper": "https://url.to.more.details.example.com/"}]
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="error.errors[].sendReport">error.errors\[\].sendReport</span>

<span id="link-error.errors[].sendReport__button" class="link_button"> [link](?showone=error.errors%5B%5D.sendReport#error.errors%5B%5D.sendReport) </span><span id="error.errors[].sendReport__button" class="showhide_button" onclick="javascript:ShowHideByName('error.errors[].sendReport')">▶</span>

<div style="display:inline;">

Property Value Type: string\
Parent: `error.errors`

</div>

<div>

<div id="error.errors[].sendReport__body" class="stylepoint_body" style="display: none">

A URI for a report form used by the service to collect data about the error condition. This URI should be preloaded with parameters describing the request.

Example:

<div>

    {
      "error":{
        "errors": [{"sendReport": "https://report.example.com/"}]
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Property Ordering

Properties can be in any order within the JSON object. However, in some cases the ordering of properties can help parsers quickly interpret data and lead to better performance. One example is a pull parser in a mobile environment, where performance and memory are critical, and unnecessary parsing should be avoided.

<div>

### <span id="Kind_Property">Kind Property</span>

<span id="link-Kind_Property__button" class="link_button"> [link](?showone=Kind_Property#Kind_Property) </span><span id="Kind_Property__button" class="showhide_button" onclick="javascript:ShowHideByName('Kind_Property')">▶</span>

<div style="display:inline;">

`kind` should be the first property

</div>

<div>

<div id="Kind_Property__body" class="stylepoint_body" style="display: none">

Suppose a parser is responsible for parsing a raw JSON stream into a specific object. The `kind` property guides the parser to instantiate the appropriate object. Therefore it should be the first property in the JSON object. This only applies when objects have a `kind` property (usually found in the `data` and `items` properties).

</div>

</div>

</div>

<div>

### <span id="Items_Property">Items Property</span>

<span id="link-Items_Property__button" class="link_button"> [link](?showone=Items_Property#Items_Property) </span><span id="Items_Property__button" class="showhide_button" onclick="javascript:ShowHideByName('Items_Property')">▶</span>

<div style="display:inline;">

`items` should be the last property in the `data` object

</div>

<div>

<div id="Items_Property__body" class="stylepoint_body" style="display: none">

This allows all of the collection's properties to be read before reading each individual item. In cases where there are a lot of items, this avoids unnecessarily parsing those items when the developer only needs fields from the data.

</div>

</div>

</div>

<div>

### <span id="Property_Ordering_Example">Property Ordering Example</span>

<span id="link-Property_Ordering_Example__button" class="link_button"> [link](?showone=Property_Ordering_Example#Property_Ordering_Example) </span><span id="Property_Ordering_Example__button" class="showhide_button" onclick="javascript:ShowHideByName('Property_Ordering_Example')">▶</span>

<div>

<div id="Property_Ordering_Example__body" class="stylepoint_body" style="display: none">

<div>

    // The "kind" property distinguishes between an "album" and a "photo".
    // "Kind" is always the first property in its parent object.
    // The "items" property is the last property in the "data" object.
    {
      "data": {
        "kind": "album",
        "title": "My Photo Album",
        "description": "An album in the user's account",
        "items": [
          {
            "kind": "photo",
            "title": "My First Photo"
          }
        ]
      }
    }

</div>

</div>

</div>

</div>

</div>

<div>

## Examples

<div>

### <span id="YouTube_JSON_API">YouTube JSON API</span>

<span id="link-YouTube_JSON_API__button" class="link_button"> [link](?showone=YouTube_JSON_API#YouTube_JSON_API) </span><span id="YouTube_JSON_API__button" class="showhide_button" onclick="javascript:ShowHideByName('YouTube_JSON_API')">▶</span>

<div style="display:inline;">

Here's an example of the YouTube JSON API's response object. You can learn more about YouTube's JSON API here: <https://code.google.com/apis/youtube/2.0/developers_guide_jsonc.html>.

</div>

<div>

<div id="YouTube_JSON_API__body" class="stylepoint_body" style="display: none">

<div>

    {
      "apiVersion": "2.0",
      "data": {
        "updated": "2010-02-04T19:29:54.001Z",
        "totalItems": 6741,
        "startIndex": 1,
        "itemsPerPage": 1,
        "items": [
          {
            "id": "BGODurRfVv4",
            "uploaded": "2009-11-17T20:10:06.000Z",
            "updated": "2010-02-04T06:25:57.000Z",
            "uploader": "docchat",
            "category": "Animals",
            "title": "From service dog to SURFice dog",
            "description": "Surf dog Ricochets inspirational video ...",
            "tags": [
              "Surf dog",
              "dog surfing",
              "dog",
              "golden retriever",
            ],
            "thumbnail": {
              "default": "https://i.ytimg.com/vi/BGODurRfVv4/default.jpg",
              "hqDefault": "https://i.ytimg.com/vi/BGODurRfVv4/hqdefault.jpg"
            },
            "player": {
              "default": "https://www.youtube.com/watch?v=BGODurRfVv4&feature=youtube_gdata",
              "mobile": "https://m.youtube.com/details?v=BGODurRfVv4"
            },
            "content": {
              "1": "rtsp://v5.cache6.c.youtube.com/CiILENy73wIaGQn-Vl-0uoNjBBMYDSANFEgGUgZ2aWRlb3MM/0/0/0/video.3gp",
              "5": "https://www.youtube.com/v/BGODurRfVv4?f=videos&app=youtube_gdata",
              "6": "rtsp://v7.cache7.c.youtube.com/CiILENy73wIaGQn-Vl-0uoNjBBMYESARFEgGUgZ2aWRlb3MM/0/0/0/video.3gp"
            },
            "duration": 315,
            "rating": 4.96,
            "ratingCount": 2043,
            "viewCount": 1781691,
            "favoriteCount": 3363,
            "commentCount": 1007,
            "commentsAllowed": true
          }
        ]
      }
    }

</div>

</div>

</div>

</div>

<div>

### <span id="Paging_Example">Paging Example</span>

<span id="link-Paging_Example__button" class="link_button"> [link](?showone=Paging_Example#Paging_Example) </span><span id="Paging_Example__button" class="showhide_button" onclick="javascript:ShowHideByName('Paging_Example')">▶</span>

<div style="display:inline;">

This example demonstrates how the Google search items could be represented as a JSON object, with special attention to the paging variables.

</div>

<div>

<div id="Paging_Example__body" class="stylepoint_body" style="display: none">

This sample is for illustrative purposes only. The API below does not actually exist.

Here's a sample Google search results page:\
![](jsoncstyleguide_example_01.png)\
![](jsoncstyleguide_example_02.png)

Here's a sample JSON representation of this page:

<div>

    {
      "apiVersion": "2.1",
      "id": "1",
      "data": {
        "query": "chicago style pizza",
        "time": "0.1",
        "currentItemCount": 10,
        "itemsPerPage": 10,
        "startIndex": 11,
        "totalItems": 2700000,
        "nextLink": "https://www.google.com/search?hl=en&q=chicago+style+pizza&start=20&sa=N"
        "previousLink": "https://www.google.com/search?hl=en&q=chicago+style+pizza&start=0&sa=N",
        "pagingLinkTemplate": "https://www.google.com/search/hl=en&q=chicago+style+pizza&start={index}&sa=N",
        "items": [
          {
            "title": "Pizz'a Chicago Home Page"
            // More fields for the search results
          }
          // More search results
        ]
      }
    }

</div>

Here's how each of the colored boxes from the screenshot would be represented (the background colors correspond to the colors in the images above):

- Results <span style="background-color:rgb(180, 167, 214)">11</span> - 20 of about 2,700,000 = startIndex
- Results 11 - <span style="background-color:rgb(255, 217, 102)">20</span> of about 2,700,000 = startIndex + currentItemCount - 1
- Results 11 - 20 of about <span style="background-color:rgb(246, 178, 107)">2,700,000</span> = totalItems
- <span style="background-color:rgb(234, 153, 153)">Search results</span> = items (formatted appropriately)
- <span style="background-color:rgb(182, 215, 168)">Previous/Next</span> = previousLink / nextLink
- <span style="background-color:rgb(159, 197, 232)">Numbered links in "Gooooooooooogle"</span> = Derived from "pageLinkTemplate". The developer is responsible for calculating the values for {index} and substituting those values into the "pageLinkTemplate". The pageLinkTemplate's {index} variable is calculated as follows:
  - Index \#1 = 0 \* itemsPerPage = 0
  - Index \#2 = 2 \* itemsPerPage = 10
  - Index \#3 = 3 \* itemsPerPage = 20
  - Index \#N = N \* itemsPerPage

</div>

</div>

</div>

</div>

<div>

## Appendix

<div>

### <span id="Appendix_A:_Reserved_JavaScript_Words">Appendix A: Reserved JavaScript Words</span>

<span id="link-Appendix_A:_Reserved_JavaScript_Words__button" class="link_button"> [link](?showone=Appendix_A:_Reserved_JavaScript_Words#Appendix_A:_Reserved_JavaScript_Words) </span><span id="Appendix_A:_Reserved_JavaScript_Words__button" class="showhide_button" onclick="javascript:ShowHideByName('Appendix_A:_Reserved_JavaScript_Words')">▶</span>

<div style="display:inline;">

A list of reserved JavaScript words that should be avoided in property names.

</div>

<div>

<div id="Appendix_A:_Reserved_JavaScript_Words__body" class="stylepoint_body" style="display: none">

The words below are reserved by the JavaScript language and cannot be referred to using dot notation. The list represents best knowledge of keywords at this time; the list may change or vary based on your specific execution environment.

From the [ECMAScript Language Specification 5th Edition](https://www.google.com/url?sa=D&q=http%3A%2F%2Fwww.ecma-international.org%2Fpublications%2Fstandards%2FEcma-262.htm)

<div>

``` badcode
abstract
boolean break byte
case catch char class const continue
debugger default delete do double
else enum export extends
false final finally float for function
goto
if implements import in instanceof int interface
let long
native new null
package private protected public
return
short static super switch synchronized
this throw throws transient true try typeof
var volatile void
while with
yield
```

</div>

</div>

</div>

</div>

</div>

------------------------------------------------------------------------

Except as otherwise [noted](https://code.google.com/policies.html), the content of this page is licensed under the [Creative Commons Attribution 3.0 License](https://creativecommons.org/licenses/by/3.0/), and code samples are licensed under the [Apache 2.0 License](https://www.apache.org/licenses/LICENSE-2.0).

Revision 0.9
