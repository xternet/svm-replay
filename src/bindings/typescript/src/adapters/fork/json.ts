//! JSON transport that preserves Solana u64 values.
/** JSON.parse that keeps every unsafe integer as its exact source text (Node 22 reviver context). */
export function parseExact(text:string):unknown{
    return JSON.parse(text,(_key,value,context?:{source?:string})=>
        typeof value==="number"&&!Number.isSafeInteger(value)&&context?.source!==undefined?context.source:value);
}
/** JSON.stringify that writes bigint values as exact integer literals. */
export function stringifyExact(value:unknown):string{
    return JSON.stringify(value,(_key,item)=>typeof item==="bigint"?`\u0000u64:${item}`:item).replace(/"\\u0000u64:(\d+)"/g,"$1");
}
